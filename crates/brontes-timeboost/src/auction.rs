use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use alloy_rpc_types::Filter;
use alloy_primitives::{Address, hex};
use std::sync::Arc;

use brontes_types::traits::TracingProvider;

sol!(
  IExpressLaneAuction,
  "./src/contracts/IExpressLaneAuction.json"
);


// TODO(jinmel): Parametrize this address
pub const ONE_EXPRESS_LANE_AUCTION_ADDRESS: Address = Address::new(hex!("5fcb496a31b7AE91e7c9078Ec662bd7A55cd3079"));

pub struct ExpressLaneControllerInfo {
  pub round: u64,
  pub controller: Address,
}
pub struct ExpressLaneAuction<T: TracingProvider> {
  provider: Arc<T>,
  contract_address: Address,
}

impl<T: TracingProvider> Clone for ExpressLaneAuction<T> {
  fn clone(&self) -> Self {
    Self { provider: self.provider.clone(), contract_address: self.contract_address.clone() }
  }
}

impl<T: TracingProvider> ExpressLaneAuction<T> {
  pub fn new(provider: Arc<T>) -> Self {
    Self { provider, contract_address: ONE_EXPRESS_LANE_AUCTION_ADDRESS }
  }

  pub async fn check_express_lane_controller(&self, block_number: u64) -> eyre::Result<Option<ExpressLaneControllerInfo>> {
    let filter = Filter::new()
      .address(self.contract_address)
      .event_signature(IExpressLaneAuction::SetExpressLaneController::SIGNATURE_HASH)
      .from_block(block_number)
      .to_block(block_number);
    let logs = self.provider.get_logs(&filter).await?;

    if logs.is_empty() {
      return Ok(None);
    }
    if logs.len() > 1 {
      tracing::warn!(?block_number, "Multiple SetExpressLaneController events found: {:?}", logs);
    }
    let log = logs.first().unwrap();
    let event = IExpressLaneAuction::SetExpressLaneController::decode_log(&log.inner, true)?;

    Ok(Some(ExpressLaneControllerInfo {
      round: event.round,
      controller: event.newExpressLaneController
    }))
  }
}

