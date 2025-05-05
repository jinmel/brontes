use async_trait::async_trait;

use crate::{
    models::{NormalizedQuote, NormalizedTrade},
    streams::{Exchange, ExchangeStream, ExchangeStreamError},
};

/// Default WebSocket URL for Binance
pub const DEFAULT_BINANCE_WS_URL: &str = "wss://stream.binance.com:9443/ws";

pub mod model;
pub mod parser;

pub struct Binance {
    symbol:   String,
    base_url: String,
}

impl Binance {
    /// Creates a new BinanceBuilder with default values
    pub fn builder() -> BinanceBuilder {
        BinanceBuilder::default()
    }
}

#[async_trait]
impl Exchange for Binance {
    type QuoteStream = ExchangeStream<NormalizedQuote>;
    type TradeStream = ExchangeStream<NormalizedTrade>;

    async fn normalized_trades(&self) -> Result<Self::TradeStream, ExchangeStreamError> {
        let url = format!("{}/{}@trade", self.base_url, self.symbol);
        ExchangeStream::new(&url, parser::parse_binance_trade).await
    }

    async fn normalized_quotes(&self) -> Result<Self::QuoteStream, ExchangeStreamError> {
        let url = format!("{}/{}@bookTicker", self.base_url, self.symbol);
        ExchangeStream::new(&url, parser::parse_binance_quote).await
    }
}
/// Builder for the Binance struct
pub struct BinanceBuilder {
    symbol:   Option<String>,
    base_url: String,
}

impl Default for BinanceBuilder {
    fn default() -> Self {
        Self { symbol: None, base_url: DEFAULT_BINANCE_WS_URL.to_string() }
    }
}

impl BinanceBuilder {
    /// Set the trading symbol
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Build the Binance instance
    pub fn build(self) -> eyre::Result<Binance> {
        let symbol = self
            .symbol
            .ok_or_else(|| eyre::eyre!("Symbol is required"))?;

        Ok(Binance { symbol, base_url: self.base_url })
    }
}
