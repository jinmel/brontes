use crate::{
    models::{NormalizedQuote, NormalizedTrade},
    streams::{binance::model::{TradeEvent, BookTickerEvent}, ExchangeStreamError},
    models::TradeSide,
};

pub fn parse_binance_trade(event: &str) -> Result<NormalizedTrade, ExchangeStreamError> {
    let trade: TradeEvent =
        serde_json::from_str(event).map_err(|e| ExchangeStreamError::ParseError(e.to_string()))?;

    // Parse price and quantity strings to f64
    let price = trade.price.parse::<f64>()
        .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid price value: {}", e)))?;
    
    let amount = trade.quantity.parse::<f64>()
        .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid quantity value: {}", e)))?;
    
    Ok(NormalizedTrade{
      exchange: "binance".as_bytes().try_into().unwrap(),
      symbol: trade.symbol.as_bytes().try_into().unwrap(),
      timestamp: trade.event_time,
      side: if trade.is_buyer_maker {
        TradeSide::Sell
      } else {
        TradeSide::Buy
      },
      price,
      amount,
    })
}

pub fn parse_binance_quote(event: &str) -> Result<NormalizedQuote, ExchangeStreamError> {
  let quote: BookTickerEvent =
        serde_json::from_str(event).map_err(|e| ExchangeStreamError::ParseError(e.to_string()))?;

  // Get current timestamp
  let timestamp = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map_err(|e| ExchangeStreamError::ParseError(format!("Failed to get timestamp: {}", e)))?
      .as_micros() as u64;
  
  // Parse ask and bid values
  let ask_amount = quote.best_ask_qty.parse::<f64>()
      .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid ask quantity value: {}", e)))?;
  
  let ask_price = quote.best_ask.parse::<f64>()
      .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid ask price value: {}", e)))?;
  
  let bid_price = quote.best_bid.parse::<f64>()
      .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid bid price value: {}", e)))?;
  
  let bid_amount = quote.best_bid_qty.parse::<f64>()
      .map_err(|e| ExchangeStreamError::ParseError(format!("Invalid bid quantity value: {}", e)))?;

  Ok(NormalizedQuote{
    exchange: "binance".as_bytes().try_into().unwrap(),
    symbol: quote.symbol.as_bytes().try_into().unwrap(),
    timestamp,
    ask_amount,
    ask_price,
    bid_price,
    bid_amount,
  })
}
