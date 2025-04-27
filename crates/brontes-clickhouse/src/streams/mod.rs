pub mod binance;

use eyre::Result;
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::Stream;
use async_trait::async_trait;

#[async_trait]
pub trait StreamConnection {
    type Error: std::error::Error + Send + Sync + 'static; 

    async fn connect(&mut self) -> Result<(), Self::Error>;
    async fn disconnect(&mut self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait DataStream<T, C> where C: StreamConnection {
    async fn get(&self) -> Result<impl Stream<Item = Result<T, C::Error>> + Send + '_, C::Error>;
}

#[async_trait]
pub trait Exchange<C> where C: StreamConnection {
    async fn normalized_trades(&self) -> Result<impl Stream<Item = Result<NormalizedTrade, C::Error>> + Send + '_, C::Error>;
    async fn normalized_quotes(&self) -> Result<impl Stream<Item = Result<NormalizedQuote, C::Error>> + Send + '_, C::Error>;
}