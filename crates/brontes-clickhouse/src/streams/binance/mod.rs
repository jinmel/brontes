use crate::streams::{Exchange, StreamConnectionError};
use crate::models::{NormalizedTrade, NormalizedQuote, TradeSide};
use futures::stream::{BoxStream, StreamExt as FuturesStreamExt};
use async_trait::async_trait;
use eyre::Result;
use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::http::Uri;
use futures::stream::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt as TokioStreamExt;

async fn connect_to_binance_quotes() -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, StreamConnectionError> {
    let url = "wss://fstream.binance.com/ws/btc@arr".parse::<Uri>()
        .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))?;
    let (ws_stream, _) = connect_async(url).await
        .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))?;
    Ok(ws_stream)
}

async fn connect_to_binance_trades() -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, StreamConnectionError> {
    let url = "wss://fstream.binance.com/ws/btc@arr".parse::<Uri>()
        .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))?;
    let (ws_stream, _) = connect_async(url).await
        .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))?;
    Ok(ws_stream)
}

pub struct Binance {
    normalized_trades: Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    normalized_quotes: Arc<Mutex<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    trade_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Drop for Binance {
    fn drop(&mut self) {
        // Get a block_in_place runtime to handle the async cleanup
        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            // Abort all running tasks
            let mut handles = rt.block_on(self.trade_handles.lock());
            for handle in handles.iter() {
                handle.abort();
            }
            handles.clear();
        });
    }
}

impl Binance {
    pub async fn new() -> Result<Self, StreamConnectionError> {
        let normalized_trades = connect_to_binance_trades().await?;
        let normalized_quotes = connect_to_binance_quotes().await?;
        
        Ok(Self {
            normalized_trades: Arc::new(Mutex::new(normalized_trades)),
            normalized_quotes: Arc::new(Mutex::new(normalized_quotes)),
            trade_handles: Arc::new(Mutex::new(Vec::new())),
        })
    }
}

#[async_trait]
impl Exchange<MaybeTlsStream<TcpStream>> for Binance {
    async fn normalized_trades(&self) -> Result<BoxStream<'_, Result<NormalizedTrade, StreamConnectionError>>, StreamConnectionError> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let stream = self.normalized_trades.clone();
        
        let handle = tokio::spawn(async move {
            let mut stream = stream.lock().await;
            while let Some(msg) = FuturesStreamExt::next(&mut *stream).await {
                match msg {
                    Ok(msg) => {
                        // TODO: Parse Binance response into NormalizedTrade
                        // For now, send an error since parsing is not implemented
                        let _ = tx.send(Err(StreamConnectionError::ConnectionFailed("Parsing not implemented".to_string()))).await;
                    },
                    Err(e) => {
                        let _ = tx.send(Err(StreamConnectionError::ConnectionFailed(e.to_string()))).await;
                    }
                }
            }
        });

        let mut handles = self.trade_handles.lock().await;
        handles.push(handle);
        
        let stream = ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn normalized_quotes(&self) -> Result<BoxStream<'_, Result<NormalizedQuote, StreamConnectionError>>, StreamConnectionError> {
        // TODO: Implement normalized quotes stream
        // TODO: Parse Binance response into NormalizedQuote
        unimplemented!("Implement normalized quotes stream")
    }
}
