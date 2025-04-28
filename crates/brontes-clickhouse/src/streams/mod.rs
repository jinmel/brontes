pub mod binance;

use eyre::Result;
use tokio_tungstenite::WebSocketStream;
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::Stream;
use async_trait::async_trait;
use std::fmt;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub enum StreamConnectionError {
    ConnectionFailed(String),
    DisconnectionFailed(String),
    StreamError(String),
    StreamNotConnected(String),
}

impl std::error::Error for StreamConnectionError {}

impl fmt::Display for StreamConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamConnectionError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            StreamConnectionError::DisconnectionFailed(msg) => write!(f, "Disconnection failed: {}", msg),
            StreamConnectionError::StreamError(msg) => write!(f, "Stream error: {}", msg),
            StreamConnectionError::StreamNotConnected(msg) => write!(f, "Stream not connected: {}", msg),
        }
    }
}

#[async_trait]
pub trait StreamConnection<T> where T: AsyncRead + AsyncWrite + Unpin
{
    type Error: std::error::Error + Send + Sync + 'static; 

    async fn connect<'a>(&'a mut self) -> Result<&'a WebSocketStream<T>, Self::Error>;
    async fn disconnect(&mut self) -> Result<(), Self::Error>;
    async fn get_stream<'a>(&'a self) -> Result<&'a WebSocketStream<T>, Self::Error>;
}

#[async_trait]
pub trait Exchange<T> where T: AsyncRead + AsyncWrite + Unpin {
    async fn normalized_trades(&self) -> Result<impl Stream<Item = Result<NormalizedTrade, StreamConnectionError>> + Send + '_, StreamConnectionError>;
    async fn normalized_quotes(&self) -> Result<impl Stream<Item = Result<NormalizedQuote, StreamConnectionError>> + Send + '_, StreamConnectionError>;
}