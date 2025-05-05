pub mod binance;

use tokio_stream::wrappers::ReceiverStream;
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::{Stream, StreamExt};
use async_trait::async_trait;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExchangeStreamError {
    #[error("Stream error: {0}")]
    StreamError(String),
    #[error("Stream not connected: {0}")]
    StreamNotConnected(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

pub struct ExchangeStream<T: Send + 'static> {
    inner: ReceiverStream<Result<T, ExchangeStreamError>>,
    handle: tokio::task::JoinHandle<()>,
}

impl<T: Send + 'static> ExchangeStream<T> {
    pub async fn new<F>(
        url: &str,
        parser: F,
    ) -> Result<Self, ExchangeStreamError>
    where
        F: Fn(&str) -> Result<T, ExchangeStreamError> + Send + Sync + 'static,
    {
        let (ws, resp) = connect_async(url)
            .await
            .map_err(|e| ExchangeStreamError::StreamError(e.to_string()))?;

        tracing::info!("Connected to {}", url);
        tracing::info!("Response: {:?}", resp);

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let handle = tokio::spawn(async move {
            ws.filter_map(|msg| {
                let parser = &parser;
                async move {
                    match msg {
                        Ok(Message::Text(text)) => {
                            Some(parser(&text))
                        }
                        Ok(Message::Close(frame)) => {
                            Some(Err(ExchangeStreamError::StreamNotConnected(format!("Close frame: {:?}", frame))))
                        }
                        // Ping/pong handled automatically
                        Ok(_) => {
                            None
                        }
                        Err(e) => {
                            Some(Err(ExchangeStreamError::StreamError(e.to_string())))
                        }
                    }
                }
            }).for_each(|parsed| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(parsed).await;
                }
            }).await;
        });

        Ok(Self { inner: ReceiverStream::new(rx), handle })
    }

    pub fn shutdown(self) {
        self.handle.abort();
    }
}

impl<T: Send + 'static> Stream for ExchangeStream<T> {
    type Item = Result<T, ExchangeStreamError>;

    fn poll_next(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl<T: Send + 'static> Drop for ExchangeStream<T> {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

#[async_trait]
pub trait Exchange {
    type TradeStream: Stream<Item = Result<NormalizedTrade, ExchangeStreamError>> + Send + Unpin + 'static;
    type QuoteStream: Stream<Item = Result<NormalizedQuote, ExchangeStreamError>> + Send + Unpin + 'static;

    async fn normalized_trades(&self) -> Result<Self::TradeStream, ExchangeStreamError>;
    async fn normalized_quotes(&self) -> Result<Self::QuoteStream, ExchangeStreamError>;
}