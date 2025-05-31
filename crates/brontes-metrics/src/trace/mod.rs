use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use metrics::{Counter, Gauge};
use reth_metrics::Metrics;
use reth_primitives::Address;
use tracing::trace;
pub mod types;
pub mod utils;
use prometheus::{opts, register_int_gauge_vec, IntGaugeVec};

use super::TraceMetricEvent;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimeSpan {
    Block,
    Day,
    Week,
    Month,
}

impl fmt::Display for TimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeSpan::Block => write!(f, "block"),
            TimeSpan::Day => write!(f, "daily"),
            TimeSpan::Week => write!(f, "weekly"),
            TimeSpan::Month => write!(f, "monthly"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceMetrics {
    txs: HashMap<String, TransactionTracingMetrics>,
    // Store unique EOA addresses for each time span
    eoa_addresses: HashMap<TimeSpan, HashSet<Address>>,
    // Store the last processed block number for each time span to detect rollovers
    last_processed_block: HashMap<TimeSpan, u64>,
    aggregated_unique_eoa_count: IntGaugeVec,
}

impl Default for TraceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceMetrics {
    pub fn new() -> Self {
        let aggregated_unique_eoa_count_gauge = register_int_gauge_vec!(
            opts!(
                "aggregated_unique_eoa_count_by_interval",
                "Total count of unique active EOA addresses for a given interval"
            ),
            &["interval_type"] // e.g., "block", "daily", "weekly", "monthly"
        )
        .expect("Failed to register aggregated_unique_eoa_count_by_interval gauge");

        let mut eoa_addresses = HashMap::new();
        eoa_addresses.insert(TimeSpan::Block, HashSet::new());
        eoa_addresses.insert(TimeSpan::Day, HashSet::new());
        eoa_addresses.insert(TimeSpan::Week, HashSet::new());
        eoa_addresses.insert(TimeSpan::Month, HashSet::new());

        let mut last_processed_block = HashMap::new();
        last_processed_block.insert(TimeSpan::Block, 0);
        last_processed_block.insert(TimeSpan::Day, 0);
        last_processed_block.insert(TimeSpan::Week, 0);
        last_processed_block.insert(TimeSpan::Month, 0);

        Self {
            txs: HashMap::new(),
            eoa_addresses,
            last_processed_block,
            aggregated_unique_eoa_count: aggregated_unique_eoa_count_gauge,
        }
    }

    /// Returns existing or initializes a new instance of [LiveRelayMetrics]
    #[allow(dead_code)]
    pub(crate) fn get_transaction_metrics(
        &mut self,
        tx_hash: String,
    ) -> &mut TransactionTracingMetrics {
        self.txs.entry(tx_hash.clone()).or_insert_with(|| {
            TransactionTracingMetrics::new_with_labels(&[("transaction_tracing", tx_hash)])
        })
    }

    pub(crate) fn handle_event(&mut self, event: TraceMetricEvent) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            TraceMetricEvent::BlockMetricRecieved(block) => {
                let block_num = block.block_num;

                // Check for period rollovers based on block number
                // This is a simplified approach - in production you might want to use
                // actual timestamps from block headers
                self.check_and_handle_rollovers(block_num);

                // For per-block metrics, clear and update
                self.eoa_addresses
                    .get_mut(&TimeSpan::Block)
                    .unwrap()
                    .clear();
                self.eoa_addresses
                    .get_mut(&TimeSpan::Block)
                    .unwrap()
                    .extend(&block.eoa_addresses);

                // Add to aggregated sets
                self.eoa_addresses
                    .get_mut(&TimeSpan::Day)
                    .unwrap()
                    .extend(&block.eoa_addresses);
                self.eoa_addresses
                    .get_mut(&TimeSpan::Week)
                    .unwrap()
                    .extend(&block.eoa_addresses);
                self.eoa_addresses
                    .get_mut(&TimeSpan::Month)
                    .unwrap()
                    .extend(&block.eoa_addresses);

                // Update metrics
                self.set_aggregated_unique_eoa_count(
                    TimeSpan::Block,
                    self.eoa_addresses[&TimeSpan::Block].len() as u64,
                );
                self.set_aggregated_unique_eoa_count(
                    TimeSpan::Day,
                    self.eoa_addresses[&TimeSpan::Day].len() as u64,
                );
                self.set_aggregated_unique_eoa_count(
                    TimeSpan::Week,
                    self.eoa_addresses[&TimeSpan::Week].len() as u64,
                );
                self.set_aggregated_unique_eoa_count(
                    TimeSpan::Month,
                    self.eoa_addresses[&TimeSpan::Month].len() as u64,
                );

                // Update last processed block
                self.last_processed_block.insert(TimeSpan::Block, block_num);
            }
            _ => {}
        }
    }

    fn check_and_handle_rollovers(&mut self, current_block: u64) {
        // Simplified rollover detection based on block numbers
        // Assuming ~0.25 second blocks on Ethereum mainnet:
        // - Daily: ~345600 blocks
        // - Weekly: ~2419200 blocks
        // - Monthly: ~10368000 blocks (30 days)

        const BLOCKS_PER_DAY: u64 = 345600;
        const BLOCKS_PER_WEEK: u64 = 2419200;
        const BLOCKS_PER_MONTH: u64 = 10368000;

        let last_day_block = self.last_processed_block[&TimeSpan::Day];
        let last_week_block = self.last_processed_block[&TimeSpan::Week];
        let last_month_block = self.last_processed_block[&TimeSpan::Month];

        // Check daily rollover
        if last_day_block > 0 && current_block / BLOCKS_PER_DAY > last_day_block / BLOCKS_PER_DAY {
            trace!("Daily rollover detected at block {}", current_block);
            self.eoa_addresses.get_mut(&TimeSpan::Day).unwrap().clear();
            self.last_processed_block
                .insert(TimeSpan::Day, current_block);
        } else if last_day_block == 0 {
            self.last_processed_block
                .insert(TimeSpan::Day, current_block);
        }

        // Check weekly rollover
        if last_week_block > 0
            && current_block / BLOCKS_PER_WEEK > last_week_block / BLOCKS_PER_WEEK
        {
            trace!("Weekly rollover detected at block {}", current_block);
            self.eoa_addresses.get_mut(&TimeSpan::Week).unwrap().clear();
            self.last_processed_block
                .insert(TimeSpan::Week, current_block);
        } else if last_week_block == 0 {
            self.last_processed_block
                .insert(TimeSpan::Week, current_block);
        }

        // Check monthly rollover
        if last_month_block > 0
            && current_block / BLOCKS_PER_MONTH > last_month_block / BLOCKS_PER_MONTH
        {
            trace!("Monthly rollover detected at block {}", current_block);
            self.eoa_addresses
                .get_mut(&TimeSpan::Month)
                .unwrap()
                .clear();
            self.last_processed_block
                .insert(TimeSpan::Month, current_block);
        } else if last_month_block == 0 {
            self.last_processed_block
                .insert(TimeSpan::Month, current_block);
        }
    }

    /// Call this method from your aggregation logic to set daily, weekly, etc.
    /// counts.
    pub fn set_aggregated_unique_eoa_count(&self, interval_name: TimeSpan, count: u64) {
        self.aggregated_unique_eoa_count
            .with_label_values(&[&interval_name.to_string()])
            .set(count as i64);
    }
}
#[allow(dead_code)]
#[derive(Metrics, Clone)]
#[metrics(scope = "transaction_tracing")]
pub(crate) struct TransactionTracingMetrics {
    /// The block number currently on
    pub(crate) block_num: Gauge,
    /// The transaction index in the block
    pub(crate) tx_idx: Gauge,
    /// The trace index in the transaction
    pub(crate) tx_trace_idx: Gauge,
    /// The total amount of successful traces for this Transaction hash
    pub(crate) success_traces: Counter,
    /// The total amount of trace errors for this Transaction hash
    pub(crate) error_traces: Counter,
    /// Empty Input Errors
    pub(crate) empty_input_errors: Counter,
    /// Abi Parse Errors
    pub(crate) abi_parse_errors: Counter,
    /// Invalid Function Selector Errors
    pub(crate) invalid_function_selector_errors: Counter,
    /// Abi Decoding Failed Errors
    pub(crate) abi_decoding_failed_errors: Counter,
    /// Trace Missing Errors
    pub(crate) block_trace_missing_errors: Counter,
    /// Trace Missing Errors
    pub(crate) tx_trace_missing_errors: Counter,
    /// Etherscan Chain Not Supported
    pub(crate) etherscan_chain_not_supported: Counter,
    /// Etherscan Execution Failed
    pub(crate) etherscan_execution_failed: Counter,
    /// Etherscan Balance Failed
    pub(crate) etherscan_balance_failed: Counter,
    /// Etherscan Not Proxy
    pub(crate) etherscan_not_proxy: Counter,
    /// Etherscan Missing Implementation Address
    pub(crate) etherscan_missing_implementation_address: Counter,
    /// Etherscan Block Number By Timestamp Failed
    pub(crate) etherscan_block_number_by_timestamp_failed: Counter,
    /// Etherscan Transaction Receipt Failed
    pub(crate) etherscan_transaction_receipt_failed: Counter,
    /// Etherscan Gas Estimation Failed
    pub(crate) etherscan_gas_estimation_failed: Counter,
    /// Etherscan Bad Status Code
    pub(crate) etherscan_bad_status_code: Counter,
    /// Etherscan Env Var Not Found
    pub(crate) etherscan_env_var_not_found: Counter,
    /// Etherscan Reqwest
    pub(crate) etherscan_reqwest: Counter,
    /// Etherscan Serde
    pub(crate) etherscan_serde: Counter,
    /// Etherscan Contract Code Not Verified
    pub(crate) etherscan_contract_code_not_verified: Counter,
    /// Etherscan Empty Result
    pub(crate) etherscan_empty_result: Counter,
    /// Etherscan Rate Limit Exceeded
    pub(crate) etherscan_rate_limit_exceeded: Counter,
    /// Etherscan Io
    pub(crate) etherscan_io: Counter,
    /// Etherscan Local Networks Not Supported
    pub(crate) etherscan_local_networks_not_supported: Counter,
    /// Etherscan Error Response
    pub(crate) etherscan_error_response: Counter,
    /// Etherscan Unknown
    pub(crate) etherscan_unknown: Counter,
    /// Etherscan Builder Error
    pub(crate) etherscan_builder: Counter,
    /// Etherscan Missing Solc Version Error
    pub(crate) etherscan_missing_solc_version: Counter,
    /// Etherscan Invalid API Key Error
    pub(crate) etherscan_invalid_api_key: Counter,
    /// Etherscan Blocked By Cloudflare Error
    pub(crate) etherscan_blocked_by_cloudflare: Counter,
    /// Etherscan Cloudflair Security Challenge Error
    pub(crate) etherscan_cloudflare_security_challenge: Counter,
    /// Etherscan Page Not Found Error
    pub(crate) etherscan_page_not_found: Counter,
    /// Etherscan Cache Error
    pub(crate) etherscan_cache_error: Counter,
    /// Etherscan Cache Error
    pub(crate) eth_api_error: Counter,
    /// EthApi Empty Raw Transaction Data Errors
    pub(crate) eth_api_empty_raw_transaction_data: Counter,
    /// EthApi Failed To Decode Signed Transaction Errors
    pub(crate) eth_api_failed_to_decode_signed_transaction: Counter,
    /// EthApi Invalid Transaction Signature Errors
    pub(crate) eth_api_invalid_transaction_signature: Counter,
    /// EthApi Pool Error
    pub(crate) eth_api_pool_error: Counter,
    /// EthApi Unknown Block Number Errors
    pub(crate) eth_api_unknown_block_number: Counter,
    /// EthApi Unknown Block Or Tx Index Errors
    pub(crate) eth_api_unknown_block_or_tx_index: Counter,
    /// EthApi Invalid Block Range Errors
    pub(crate) eth_api_invalid_block_range: Counter,
    /// EthApi Prevrandao Not Set Errors
    pub(crate) eth_api_prevrandao_not_set: Counter,
    /// EthApi Conflicting Fee Fields In Request Errors
    pub(crate) eth_api_conflicting_fee_fields_in_request: Counter,
    /// EthApi Invalid Transaction Errors
    pub(crate) eth_api_invalid_transaction: Counter,
    /// EthApi Invalid Block Data Errors
    pub(crate) eth_api_invalid_block_data: Counter,
    /// EthApi Both State And State Diff In Override Errors
    pub(crate) eth_api_both_state_and_state_diff_in_override: Counter,
    /// EthApi Internal Errors
    pub(crate) eth_api_internal: Counter,
    /// EthApi Signing Errors
    pub(crate) eth_api_signing: Counter,
    /// EthApi Transaction Not Found Errors
    pub(crate) eth_api_transaction_not_found: Counter,
    /// EthApi Unsupported Errors
    pub(crate) eth_api_unsupported: Counter,
    /// EthApi Invalid Params Errors
    pub(crate) eth_api_invalid_params: Counter,
    /// EthApi Invalid Tracer Config Errors
    pub(crate) eth_api_invalid_tracer_config: Counter,
    /// EthApi Invalid Reward Percentiles Errors
    pub(crate) eth_api_invalid_reward_percentiles: Counter,
    /// EthApi Internal Tracing Error
    pub(crate) eth_api_internal_tracing_error: Counter,
    /// EthApi Internal Eth Error
    pub(crate) eth_api_internal_eth_error: Counter,
    /// EthApi Internal Js Tracer Error
    pub(crate) eth_api_internal_js_tracer_error: Counter,
    /// EthApi Unkown Safe or Finalised Block Error
    pub(crate) eth_api_unknown_safe_or_finalized_block: Counter,
    /// EthApi Execution Timed Out Error
    pub(crate) eth_api_execution_timed_out: Counter,
    /// EthApi Call Input Error
    pub(crate) eth_api_call_input_error: Counter,
    /// alloy error
    pub(crate) alloy_error: Counter, // todo: expand error type
}
