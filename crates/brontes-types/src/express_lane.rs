use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, Default)]
pub struct ExpressLaneMetaData {
  pub round: u64,
  pub bidder: Option<Address>,
  pub controller: Address,
  pub bid_price: Option<U256>, // first price
  pub price: Option<U256>, // second price
  pub round_start_timestamp: u64,
  pub round_end_timestamp: u64,
  pub block_number: u64,
}