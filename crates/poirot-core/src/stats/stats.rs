use std::{sync::{atomic::Ordering, Mutex}, collections::HashMap};

use crate::{errors::TraceParseError, *, stats::{display::ErrorStats, format_color}};
use alloy_etherscan::errors::EtherscanError;
use colored::Color;
use revm_primitives::B256;
use serde_json::{Value, json};
use serde::Serialize;
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Id, Subscriber, info, Level,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, EnvFilter, FmtSubscriber};
use lazy_static::*;
pub struct ParserStatsLayer;


lazy_static! {
    pub static ref BLOCK_STATS: Mutex<HashMap<u64, BlockStats>> = {
        Mutex::new(HashMap::new())
    };

    pub static ref TX_STATS: Mutex<HashMap<B256, TransactionStats>> = {
        Mutex::new(HashMap::new())
    };
}

pub struct BlockStats {
    pub block_num: u64,
    pub tx_stats: Vec<TransactionStats>,
}

impl BlockStats {
    pub fn display_stats(&self) {

        println!("{}", format_color("STATS FOR BLOCK", self.block_num as usize, Color::BrightBlue).bold());
        println!("----------------------------------------------------------------------------------------");
        println!("{}", format_color("Total Transactions", self.tx_stats.len(), Color::Blue));
        println!("{}", format_color("Total Traces", self.tx_stats.iter().map(|tx| tx.error_parses.len() + tx.successful_parses).sum::<usize>(), Color::Blue));
        println!("{}", format_color("Successful Parses", self.tx_stats.iter().map(|tx| tx.successful_parses).sum::<usize>(), Color::Blue));
        println!("{}", format_color("Total Errors", self.tx_stats.iter().map(|tx| tx.error_parses.len()).sum::<usize>(), Color::Blue));

        let mut errors = ErrorStats::default();
        for err in self.tx_stats.iter().map(|tx| &tx.error_parses).flatten() {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Blue, "");
        println!();
    }
}

pub struct TransactionStats {
    pub tx_hash: B256,
    pub successful_parses: usize,
    pub error_parses: Vec<TraceStat>,
}

impl TransactionStats {
    pub fn display_stats(&self) {
        let spacing = " ".repeat(8);

        println!("{}{}", spacing, format_color("STATS FOR TRANSACTION", format!("{:#x}", self.tx_hash), Color::BrightCyan).bold());
        println!("{}----------------------------------------------------------------------------------------", spacing);
        println!("{}{}", spacing, format_color("Total Traces", self.successful_parses + self.error_parses.len(), Color::Cyan));
        println!("{}{}", spacing, format_color("Successful Parses", self.successful_parses, Color::Cyan));
        println!("{}{}", spacing, format_color("Total Errors", self.error_parses.len(), Color::Cyan));

        let mut errors = ErrorStats::default();
        for err in &self.error_parses {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Cyan, &spacing);

        for trace in &self.error_parses {
            println!("{}{}", spacing.repeat(1), format!("{} - {:?}", format_color("Error - Trace", trace.idx, Color::Cyan), trace.error));
        }
        println!();
    }
}

pub struct TraceStat {
    pub idx: usize,
    pub error: Box<dyn std::error::Error + Sync + Send +'static>
}
