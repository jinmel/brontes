pub mod binance;

use tokio_stream::wrappers::ReceiverStream;
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::{Stream, StreamExt};
use async_trait::async_trait;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StreamConnectionError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Disconnection failed: {0}")]
    DisconnectionFailed(String),
    #[error("Stream error: {0}")]
    StreamError(String),
    #[error("Stream not connected: {0}")]
    StreamNotConnected(String),
}

pub struct ExchangeStream<T: Send + 'static> {
    inner: ReceiverStream<Result<T, StreamConnectionError>>,
    handle: tokio::task::JoinHandle<()>,
}

impl<T: Send + 'static> ExchangeStream<T> {
    pub async fn new<F>(
        url: &str,
        parser: F,
    ) -> Result<Self, StreamConnectionError>
    where
        F: Fn(Message) -> Result<T, StreamConnectionError> + Send + Sync + 'static,
    {
        let (mut ws, _) = connect_async(url)
            .await
            .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        let handle = tokio::spawn(async move {
            while let Some(msg) = ws.next().await {
                let out = msg
                    .map_err(|e| StreamConnectionError::ConnectionFailed(e.to_string()))
                    .and_then(&parser);
                if tx.send(out).await.is_err() {
                    break;
                }
            }
        });

        Ok(Self { inner: ReceiverStream::new(rx), handle })
    }

    pub fn shutdown(self) {
        self.handle.abort();
    }
}

impl<T: Send + 'static> Stream for ExchangeStream<T> {
    type Item = Result<T, StreamConnectionError>;

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
    type TradeStream: Stream<Item = Result<NormalizedTrade, StreamConnectionError>> + Send + Unpin + 'static;
    type QuoteStream: Stream<Item = Result<NormalizedQuote, StreamConnectionError>> + Send + Unpin + 'static;

    async fn normalized_trades(&self) -> Result<Self::TradeStream, StreamConnectionError>;
    async fn normalized_quotes(&self) -> Result<Self::QuoteStream, StreamConnectionError>;
}

// For backwards compatibility, keep spawn_ws_stream as a thin wrapper around ExchangeStream::new
async fn spawn_ws_stream<T, F>(
    url: &str,
    parser: F,
) -> Result<ExchangeStream<T>, StreamConnectionError>
where
    T: Send + 'static,
    F: Fn(Message) -> Result<T, StreamConnectionError> + Send + Sync + 'static,
{
    ExchangeStream::new(url, parser).await
}