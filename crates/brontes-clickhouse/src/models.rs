use serde::{Deserialize, Serialize};
use clickhouse::Row;

#[derive(Debug)]
pub enum NormalizedEvent {
    Trade(NormalizedTrade),
    Quote(NormalizedQuote),
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TradeSide {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Row)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTrade {
    pub exchange:  [u8; 16],    
    pub symbol:    [u8; 16],    
    pub timestamp: u64,       
    pub side:      TradeSide, 
    pub price:     f64,
    pub amount:    f64,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone, Row)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedQuote {
    pub exchange:   [u8; 16],
    pub symbol:     [u8; 16],
    pub timestamp:  u64,
    pub ask_amount: f64,
    pub ask_price:  f64,
    pub bid_price:  f64,
    pub bid_amount: f64,
}

impl NormalizedTrade {
    pub fn new(exchange: &str, symbol: &str, timestamp: u64, side: TradeSide, price: f64, amount: f64) -> Self {
        let mut exchange_bytes = [0u8; 16];
        let mut symbol_bytes = [0u8; 16];
        exchange_bytes[..exchange.len().min(16)].copy_from_slice(exchange.as_bytes());
        symbol_bytes[..symbol.len().min(16)].copy_from_slice(symbol.as_bytes());
        
        Self {
            exchange: exchange_bytes,
            symbol: symbol_bytes,
            timestamp,
            side,
            price,
            amount,
        }
    }
    
    pub fn exchange(&self) -> &str {
        std::str::from_utf8(&self.exchange)
            .unwrap_or("")
            .trim_end_matches('\0')
    }
    
    pub fn symbol(&self) -> &str {
        std::str::from_utf8(&self.symbol)
            .unwrap_or("")
            .trim_end_matches('\0')
    }
}

impl NormalizedQuote {
    pub fn new(exchange: &str, symbol: &str, timestamp: u64, ask_amount: f64, ask_price: f64, bid_price: f64, bid_amount: f64) -> Self {
        let mut exchange_bytes = [0u8; 16];
        let mut symbol_bytes = [0u8; 16];
        exchange_bytes[..exchange.len().min(16)].copy_from_slice(exchange.as_bytes());
        symbol_bytes[..symbol.len().min(16)].copy_from_slice(symbol.as_bytes());
        
        Self {
            exchange: exchange_bytes,
            symbol: symbol_bytes,
            timestamp,
            ask_amount,
            ask_price,
            bid_price,
            bid_amount,
        }
    }
    
    pub fn exchange(&self) -> &str {
        std::str::from_utf8(&self.exchange)
            .unwrap_or("")
            .trim_end_matches('\0')
    }
    
    pub fn symbol(&self) -> &str {
        std::str::from_utf8(&self.symbol)
            .unwrap_or("")
            .trim_end_matches('\0')
    }
}
