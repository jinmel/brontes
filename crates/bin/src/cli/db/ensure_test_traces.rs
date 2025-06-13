use std::path::Path;

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::ParserMetricsListener;
use brontes_types::{
    chain_config::ChainConfig, init_thread_pools, unordered_buffer_map::BrontesStreamExt, UnboundedYapperReceiver
};
use clap::Parser;
use futures::StreamExt;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, get_env_vars, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct TestTraceArgs {
    /// Blocks to trace
    #[arg(long, short, value_delimiter = ',')]
    pub blocks: Vec<u64>,

    /// Chain Id or Name
    #[arg(long, short, default_value = "arbitrum")]
    pub chain: String,
}

impl TestTraceArgs {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;
        let chain_config = ChainConfig::new(self.chain.to_owned())?;

        let max_tasks = determine_max_tasks(None) * 2;
        init_thread_pools(max_tasks as usize);
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = ParserMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));
        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let libmdbx =
            static_object(load_database(&ctx.task_executor, chain_config, brontes_db_path, None, None).await?);

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        futures::stream::iter(self.blocks.into_iter())
            .unordered_buffer_map(100, |i| parser.execute(i, 0, None))
            .map(|_res| ())
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }
}
