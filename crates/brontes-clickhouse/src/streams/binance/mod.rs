use crate::streams::{StreamConnection, DataStream, Exchange};
use crate::models::{NormalizedTrade, NormalizedQuote};
use futures::stream::{Stream, BoxStream};
use async_trait::async_trait;
use eyre::Result;
use std::error::Error as StdError;
use std::io;

pub struct BinanceStream;

#[async_trait]
impl StreamConnection for BinanceStream {
    type Error = io::Error;

    async fn connect(&mut self) -> Result<(), Self::Error> {
        // Implementation details to be added
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        // Implementation details to be added
        Ok(())
    }
}

pub struct BinanceTradeStream {
    connection: BinanceStream,
}

pub struct BinanceQuoteStream {
    connection: BinanceStream,
}

#[async_trait]
impl DataStream<NormalizedTrade, BinanceStream> for BinanceTradeStream {
    async fn get(&self) -> Result<BoxStream<'_, Result<NormalizedTrade, <BinanceStream as StreamConnection>::Error>>, <BinanceStream as StreamConnection>::Error> {
        // Implementation details to be added
        todo!()
    }
}

#[async_trait]
impl DataStream<NormalizedQuote, BinanceStream> for BinanceQuoteStream {
    async fn get(&self) -> Result<BoxStream<'_, Result<NormalizedQuote, <BinanceStream as StreamConnection>::Error>>, <BinanceStream as StreamConnection>::Error> {
        // Implementation details to be added
        todo!()
    }
}

pub struct Binance {
    normalized_trades: BinanceTradeStream,
    normalized_quotes: BinanceQuoteStream,
}

#[async_trait]
impl Exchange<BinanceStream> for Binance {
    async fn normalized_trades(&self) -> Result<BoxStream<'_, Result<NormalizedTrade, <BinanceStream as StreamConnection>::Error>>, <BinanceStream as StreamConnection>::Error> {
        self.normalized_trades.get().await
    }

    async fn normalized_quotes(&self) -> Result<BoxStream<'_, Result<NormalizedQuote, <BinanceStream as StreamConnection>::Error>>, <BinanceStream as StreamConnection>::Error> {
        self.normalized_quotes.get().await
    }
}

impl Binance {
    pub fn new(trades_stream: BinanceTradeStream, quotes_stream: BinanceQuoteStream) -> Self {
        Self {
            normalized_trades: trades_stream,
            normalized_quotes: quotes_stream,
        }
    }
}
