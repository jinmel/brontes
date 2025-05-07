use dotenv::dotenv;
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tower::ServiceExt;
use tracing_subscriber::FmtSubscriber;

use crate::{
    clickhouse::{ClickHouseConfig, ClickHouseService},
    models::NormalizedEvent,
    streams::{binance::Binance, Exchange, ExchangeStreamError},
};

mod clickhouse;
mod models;
mod streams;

const BATCH_SIZE: usize = 100;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Load environment variables from .env file
    dotenv().ok();

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let (evt_tx, evt_rx) = mpsc::channel::<Vec<Result<NormalizedEvent, ExchangeStreamError>>>(1000);

    let mut set = tokio::task::JoinSet::new();

    set.spawn(binance_stream_task(evt_tx.clone()));
    set.spawn(clickhouse_writer_task(evt_rx));

    tokio::select! {
        _ = async {
            while let Some(res) = set.join_next().await {
                match res {
                    Ok(Ok(())) => {
                        // one worker returned Ok(())
                        continue;
                    }
                    Ok(Err(err)) => {
                        tracing::error!("worker task failed: {:?}", err);
                        break;
                    }
                    Err(join_err) => {
                        tracing::error!("worker task panicked: {}", join_err);
                        break;
                    }
                }
            }
        } => {
            tracing::info!("One of the worker tasks has finished; shutting down remaining tasks");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("SIGINT received; shutting down worker tasks");
        }
    }

    // abort any remaining in-flight tasks
    set.abort_all();

    // Wait for all tasks to complete their abort
    while let Some(res) = set.join_next().await {
        if let Err(e) = res {
            tracing::warn!("Error during task shutdown: {}", e);
        }
    }
    tracing::info!("All tasks have been shut down");
    Ok(())
}

async fn binance_stream_task(
    evt_tx: mpsc::Sender<Vec<Result<NormalizedEvent, ExchangeStreamError>>>,
) -> eyre::Result<()> {
    let binance = Binance::builder().symbol("BTCUSDT").build()?;
    let trades = binance
        .normalized_trades()
        .await?
        .map(|result| result.map(NormalizedEvent::Trade));
    let quotes = binance
        .normalized_quotes()
        .await?
        .map(|result| result.map(NormalizedEvent::Quote));
    let merged = futures::stream::select(trades, quotes);
    let chunks = merged.chunks(BATCH_SIZE);

    chunks
        .for_each_concurrent(10, |batch| {
            let tx = evt_tx.clone();
            async move {
                if let Err(e) = tx.send(batch).await {
                    tracing::error!("Error sending batch to ClickHouse: {:?}", e);
                }
            }
        })
        .await;
    Ok(())
}

async fn clickhouse_writer_task(
    rx: mpsc::Receiver<Vec<Result<NormalizedEvent, ExchangeStreamError>>>,
) -> eyre::Result<()> {
    let cfg = ClickHouseConfig::from_env()?;
    let writer = ClickHouseService::new(cfg);

    let batch_stream = ReceiverStream::new(rx)
        .map(|batch| {
            batch
                .into_iter()
                .inspect(|result| {
                    if let Err(e) = result {
                        tracing::error!("{:?}", e);
                    }
                })
                .filter_map(Result::ok)
                .collect::<Vec<_>>()
        })
        .filter(|events| futures::future::ready(!events.is_empty()));

    let mut resp_stream = writer.call_all(batch_stream);
    while let Some(result) = resp_stream.next().await {
        if let Err(e) = result {
            tracing::error!("Error writing to ClickHouse: {:?}", e);
        }
    }
    Ok(())
}
