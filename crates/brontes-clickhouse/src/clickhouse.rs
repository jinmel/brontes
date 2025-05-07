use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};

use clickhouse::Client;
use futures::{Future, Sink, Stream};
use futures::SinkExt;
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::info;

use crate::models::{NormalizedEvent, NormalizedQuote, NormalizedTrade};

#[derive(Debug)]
pub struct ClickHouseConfig {
    pub url:      String,
    pub port:     String,
    pub user:     String,
    pub database: String,
    pub password: String,
}

impl ClickHouseConfig {
    pub fn from_env() -> eyre::Result<Self> {
        Ok(Self {
            url:      std::env::var("CLICKHOUSE_URL")?,
            port:     std::env::var("CLICKHOUSE_PORT")?,
            user:     std::env::var("CLICKHOUSE_USER")?,
            database: std::env::var("CLICKHOUSE_DATABASE")?,
            password: std::env::var("CLICKHOUSE_PASS")?,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}", self.url, self.port)
    }
}
pub struct ClickHouseService {
    client: Client,
}

impl Clone for ClickHouseService {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
        }
    }
}

impl ClickHouseService {
    pub fn new(config: ClickHouseConfig) -> Self {
        let client = Client::default()
            .with_url(config.url())
            .with_user(config.user)
            .with_password(config.password)
            .with_database(config.database);
        Self { client }
    }

    async fn write_trade(&self, trades: &Vec<NormalizedTrade>) -> eyre::Result<()> {
        let mut inserter = self
            .client
            .inserter("normalized_trades".to_string())?
            .with_max_entries(100);

        for trade in trades {
            inserter.write(trade).await?;
            inserter.commit().await?;
        }
        inserter.end().await?;
        Ok(())
    }

    async fn write_quote(&self, quotes: &Vec<NormalizedQuote>) -> eyre::Result<()> {
        let mut inserter = self
            .client
            .inserter("normalized_quotes".to_string())?
            .with_max_entries(100);

        for quote in quotes {
            inserter.write(quote).await?;
            inserter.commit().await?;
        }
        inserter.end().await?;
        Ok(())
    }

    async fn write_batch(&self, events: Vec<NormalizedEvent>) -> eyre::Result<()> {
        let mut trade_inserter = self
            .client
            .inserter("normalized_trades".to_string())?
            .with_max_entries(100)
            .with_period(Some(Duration::from_secs(1)))
            .with_period_bias(0.1);

        let mut quote_inserter = self
            .client
            .inserter("normalized_quotes".to_string())?
            .with_max_entries(100)
            .with_period(Some(Duration::from_secs(1)))
            .with_period_bias(0.1);

        for event in events {
            match event {
                NormalizedEvent::Trade(trade) => {
                    trade_inserter.write(&trade).await?;
                    trade_inserter.commit().await?;
                }
                NormalizedEvent::Quote(quote) => {
                    quote_inserter.write(&quote).await?;
                    quote_inserter.commit().await?;
                }
            }
        }
        trade_inserter.end().await?;
        quote_inserter.end().await?;
        Ok(())
    }
}


impl tower::Service<Vec<NormalizedEvent>> for ClickHouseService {
    type Response = ();
    type Error    = eyre::Error;
    type Future   = Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // we’re always ready
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, batch: Vec<NormalizedEvent>) -> Self::Future {
        let svc = self.clone();
        Box::pin(async move {
            // if write_batch fails, this returns Err(eyre::Error)
            svc.write_batch(batch).await
              .map_err(|e| eyre::eyre!("clickhouse write failed: {}", e))?;
            // on success:
            Ok(())
        })
    }
}