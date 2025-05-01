use crate::streams::{Exchange, StreamConnectionError};
use crate::models::{NormalizedTrade, NormalizedQuote, TradeSide};
use futures::stream::BoxStream;
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
use futures::StreamExt;
use tokio::sync::mpsc;
use std::time::Duration;
pub struct Binance {
    reconnect_interval: Duration,
    symbol: String,
}