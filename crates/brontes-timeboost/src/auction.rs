use std::{cmp::Ordering, sync::Arc};

use alloy_primitives::{hex, Address, U256};
use alloy_rpc_types::Filter;
use alloy_sol_macro::sol;
use alloy_sol_types::SolEvent;
use brontes_types::{express_lane::ExpressLaneMetaData, traits::TracingProvider};

sol!(IExpressLaneAuction, "./src/contracts/IExpressLaneAuction.json");

#[derive(Debug)]
pub enum ExpressLaneAuctionLog {
    SetExpressLaneController(ExpressLaneControllerEvent),
    AuctionResolved(ExpressLaneAuctionEvent),
}

// TODO(jinmel): Parametrize this address
pub const ONE_EXPRESS_LANE_AUCTION_ADDRESS: Address =
    Address::new(hex!("5fcb496a31b7AE91e7c9078Ec662bd7A55cd3079"));

// 250ms block time, 1min per round
pub const BLOCKS_PER_ROUND: u64 = 4 * 60;

#[derive(Debug)]
pub struct ExpressLaneControllerEvent {
    pub block_number: u64,
    pub round: u64,
    pub new_express_lane_controller: Address,
    pub previous_express_lane_controller: Address,
    pub transferor: Address,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

#[derive(Debug)]
pub struct ExpressLaneAuctionEvent {
    pub block_number: u64,
    pub round: u64,
    pub first_price_bidder: Address,
    pub first_price_express_lane_controller: Address,
    pub first_price_amount: U256,
    pub price: U256,
    pub round_start_timestamp: u64,
    pub round_end_timestamp: u64,
}

#[derive(Debug)]
pub struct ExpressLaneAuctionProvider<T: TracingProvider> {
    provider:         Arc<T>,
    contract_address: Address,
}

impl<T: TracingProvider> Clone for ExpressLaneAuctionProvider<T> {
    fn clone(&self) -> Self {
        Self { provider: self.provider.clone(), contract_address: self.contract_address }
    }
}

impl<T: TracingProvider> ExpressLaneAuctionProvider<T> {
    pub fn new(provider: Arc<T>) -> Self {
        Self { provider, contract_address: ONE_EXPRESS_LANE_AUCTION_ADDRESS }
    }

    pub async fn get_express_lane_meta_data(
        &self,
        block_number: u64,
    ) -> eyre::Result<ExpressLaneMetaData> {
        let start_block = block_number - BLOCKS_PER_ROUND;
        let end_block = block_number;
        let logs = self
            .fetch_auction_events_range(start_block, end_block)
            .await?;

        if logs.is_empty() {
            return Err(eyre::eyre!("no auction events found for start block: {:?}, end block: {:?}", start_block, end_block));
        }

        let mut express_lane_meta_data = ExpressLaneMetaData::default();
        // apply all the logs to make the latest express lane state.
        for log in logs {
            match log {
                ExpressLaneAuctionLog::SetExpressLaneController(event) => {
                    express_lane_meta_data.block_number = event.block_number;
                    express_lane_meta_data.round = event.round;
                    express_lane_meta_data.controller = event.new_express_lane_controller;
                    express_lane_meta_data.round_start_timestamp = event.start_timestamp;
                    express_lane_meta_data.round_end_timestamp = event.end_timestamp;
                }
                ExpressLaneAuctionLog::AuctionResolved(event) => {
                    express_lane_meta_data.block_number = event.block_number;
                    express_lane_meta_data.round = event.round;
                    express_lane_meta_data.controller = event.first_price_express_lane_controller;
                    express_lane_meta_data.round_start_timestamp = event.round_start_timestamp;
                    express_lane_meta_data.round_end_timestamp = event.round_end_timestamp;
                    express_lane_meta_data.bidder = Some(event.first_price_bidder);
                    express_lane_meta_data.bid_price = Some(event.first_price_amount);
                    express_lane_meta_data.price = Some(event.price);
                }
            }
        }

        Ok(express_lane_meta_data)
    }

    pub async fn fetch_auction_events(
        &self,
        block_number: u64,
    ) -> eyre::Result<Vec<ExpressLaneAuctionLog>> {
        let logs = self
            .fetch_auction_events_range(block_number, block_number)
            .await?;
        Ok(logs)
    }

    // returns all auction events in ascending order of logs.
    pub async fn fetch_auction_events_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> eyre::Result<Vec<ExpressLaneAuctionLog>> {
        let topics = vec![
            IExpressLaneAuction::SetExpressLaneController::SIGNATURE_HASH,
            IExpressLaneAuction::AuctionResolved::SIGNATURE_HASH,
        ];

        let filter = Filter::new()
            .address(self.contract_address)
            .event_signature(topics)
            .from_block(start_block)
            .to_block(end_block);
        let mut logs = self.provider.get_logs(&filter).await?;
        if logs.is_empty() {
            return Ok(vec![]);
        }

        // sort by order of logs in both block number and log_index within a block.
        logs.sort_by(|a, b| {
            let block_cmp = a.block_number.cmp(&b.block_number);
            match block_cmp {
                Ordering::Equal => a.log_index.cmp(&b.log_index),
                _ => block_cmp,
            }
        });

        let mut updates = Vec::new();
        for log in logs {
            // Check the first topic (event signature) to determine which event type this is
            if let Some(topic0) = log.topics().first() {
                if *topic0 == IExpressLaneAuction::SetExpressLaneController::SIGNATURE_HASH {
                    // Decode as SetExpressLaneController event
                    let event = IExpressLaneAuction::SetExpressLaneController::decode_log(
                        &log.inner, true,
                    )?;
                    updates.push(ExpressLaneAuctionLog::SetExpressLaneController(
                        ExpressLaneControllerEvent {
                            block_number: log
                                .block_number
                                .ok_or(eyre::eyre!("block number not found"))?,
                            round: event.round,
                            new_express_lane_controller: event.newExpressLaneController,
                            previous_express_lane_controller: event.previousExpressLaneController,
                            transferor: event.transferor,
                            start_timestamp: event.startTimestamp,
                            end_timestamp: event.endTimestamp,
                        },
                    ));
                } else if *topic0 == IExpressLaneAuction::AuctionResolved::SIGNATURE_HASH {
                    // Decode as AuctionResolved event
                    let event = IExpressLaneAuction::AuctionResolved::decode_log(&log.inner, true)?;
                    updates.push(ExpressLaneAuctionLog::AuctionResolved(ExpressLaneAuctionEvent {
                        block_number: log
                            .block_number
                            .ok_or(eyre::eyre!("block number not found"))?,
                        round: event.round,
                        first_price_bidder: event.firstPriceBidder,
                        first_price_express_lane_controller: event.firstPriceExpressLaneController,
                        first_price_amount: event.firstPriceAmount,
                        price: event.price,
                        round_start_timestamp: event.roundStartTimestamp,
                        round_end_timestamp: event.roundEndTimestamp,
                    }));
                }
            }
        }

        Ok(updates)
    }
}
