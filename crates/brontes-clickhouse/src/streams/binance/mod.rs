use crate::streams::{Exchange, StreamConnectionError};
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::{BoxStream};
use async_trait::async_trait;
use eyre::Result;
use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::http::Uri;

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
    normalized_trades: WebSocketStream<MaybeTlsStream<TcpStream>>,
    normalized_quotes: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Binance {
    pub async fn new() -> Result<Self, StreamConnectionError> {
        let normalized_trades = connect_to_binance_trades().await?;
        let normalized_quotes = connect_to_binance_quotes().await?;

        Ok(Self {
            normalized_trades,
            normalized_quotes,
        })
    }
}

#[async_trait]
impl Exchange<MaybeTlsStream<TcpStream>> for Binance {
    async fn normalized_trades(&self) -> Result<BoxStream<'_, Result<NormalizedTrade, StreamConnectionError>>, StreamConnectionError> {
        // TODO: Implement normalized trades stream
        // TODO: Parse Binance response into NormalizedTrade
        unimplemented!("Implement normalized trades stream")
    }

    async fn normalized_quotes(&self) -> Result<BoxStream<'_, Result<NormalizedQuote, StreamConnectionError>>, StreamConnectionError> {
        // TODO: Implement normalized quotes stream
        // TODO: Parse Binance response into NormalizedQuote
        unimplemented!("Implement normalized quotes stream")
    }
}
