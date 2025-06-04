use std::sync::Arc;

use brontes_pricing::make_call_request;
use alloy_primitives::{Address, Uint};
use brontes_macros::action_impl;
use brontes_types::{
    constants::arbitrum::DOLOMITE_MARGIN_ADDRESS,
    normalized_actions::{NormalizedLiquidation}, structured_trace::CallInfo, traits::TracingProvider, utils::ToScaledRational, Protocol,
};
use crate::DolomiteLiquidator;


action_impl!(
    Protocol::Dolomite,
    crate::DolomiteLiquidator::operateCall,
    Liquidation,
    [..LogLiquidate],
    call_data: true,
    logs:true,
    include_delegated_logs:true,
    |
    info: CallInfo,
    _call_data: operateCall,
    logs: DolomiteOperateCallLogs,
    db: &DB | {
        let log_data=logs.log_liquidate_field?;

        let liquidator=log_data.solidAccountOwner;
        let debtor=log_data.liquidAccountOwner;

        let held_market_idx = log_data.heldMarket;
        let owed_market_idx = log_data.owedMarket;


        // collateral market
        let collateral_tokens = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                query_dolomite_market_token(&tracer, held_market_idx)
            )
        });
        let collateral_token = collateral_tokens.get(0).ok_or_else(|| eyre::eyre!("Collateral token not found"))?;
        
        let debt_tokens = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                query_dolomite_market_token(&tracer, owed_market_idx)
            )
        });
        let debt_token = debt_tokens.get(0).ok_or_else(|| eyre::eyre!("Debt token not found"))?;

        let collateral_info = db.try_fetch_token_info(*collateral_token)?;
        let debt_info = db.try_fetch_token_info(*debt_token)?;
        let liquidated_collateral = log_data.solidHeldUpdate.deltaWei.value.to_scaled_rational(collateral_info.decimals);
        let covered_debt = log_data.solidOwedUpdate.deltaWei.value.to_scaled_rational(debt_info.decimals);

        return Ok(NormalizedLiquidation {
            protocol: Protocol::Dolomite,
            trace_index: info.trace_idx,
            pool: info.target_address,
            liquidator,
            debtor,
            collateral_asset: collateral_info,
            debt_asset: debt_info,
            covered_debt,
            liquidated_collateral,
            msg_value: info.msg_value,
        })
    }
);


pub async fn query_dolomite_market_token<T: TracingProvider>(
    tracer: &Arc<T>,
    market_id: Uint<256, 4>,
) -> Vec<Address> {
    let mut result = vec![];
    if let Ok(call_return) = make_call_request(
        DolomiteLiquidator::getMarketCall { marketId: market_id },
        tracer,
        DOLOMITE_MARGIN_ADDRESS,
        None,
    ).await {
        result.push(call_return._0.token);
    }
    result
}