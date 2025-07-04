pub mod discovery_logs_only;
pub mod discovery_only;
mod processors;
mod range;
use std::ops::RangeInclusive;

use brontes_database::libmdbx::StateToInitialize;
use brontes_metrics::{
    pricing::DexPricingMetrics,
    range::{FinishedRange, GlobalRangeMetrics},
};
use futures::{future::join_all, Stream};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
pub use processors::*;
mod shared;
use brontes_database::{clickhouse::ClickhouseHandle, Tables};
use futures::pin_mut;
use shared::multi_block_window::MultiBlockWindow;
mod tip;
use std::{
    marker::PhantomData,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::decoding::{Parser, TracingProvider};
use brontes_database::libmdbx::LibmdbxInit;
use brontes_inspect::Inspector;
use brontes_pricing::{BrontesBatchPricer, GraphManager, LoadState};
use brontes_types::{
    db::traits::LibmdbxReader, BrontesTaskExecutor, FastHashMap, UnboundedYapperReceiver,
};
use futures::{stream::FuturesUnordered, Future, StreamExt};
use indicatif::MultiProgress;
use itertools::Itertools;
pub use range::RangeExecutorWithPricing;
use reth_tasks::shutdown::GracefulShutdown;
pub use tip::TipInspector;
use tokio::{sync::mpsc::unbounded_channel, task::JoinHandle};

use self::shared::{
    dex_pricing::WaitingForPricerFuture, metadata_loader::MetadataLoader,
    state_collector::StateCollector,
};
use brontes_timeboost::auction::ExpressLaneAuctionProvider;
use crate::cli::static_object;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [0u8, 0u8, 0u8, 0u8];

pub struct BrontesRunConfig<T: TracingProvider, DB: LibmdbxInit, CH: ClickhouseHandle, P: Processor>
{
    pub range_type: RangeType,
    pub max_tasks: u64,
    pub min_batch_size: u64,
    pub quote_asset: Address,
    pub force_dex_pricing: bool,
    pub force_no_dex_pricing: bool,
    pub inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
    pub clickhouse: &'static CH,
    pub parser: &'static Parser<T, DB>,
    pub libmdbx: &'static DB,
    pub tip_db: &'static DB,
    pub cli_only: bool,
    pub metrics: bool,
    pub is_snapshot: bool,
    pub cex_window: usize,
    pub max_pending: usize,
    _p: PhantomData<P>,
}

impl<T: TracingProvider, DB: LibmdbxInit, CH: ClickhouseHandle, P: Processor>
    BrontesRunConfig<T, DB, CH, P>
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        range_type: RangeType,
        max_tasks: u64,
        min_batch_size: u64,
        quote_asset: Address,
        force_dex_pricing: bool,
        force_no_dex_pricing: bool,
        inspectors: &'static [&'static dyn Inspector<Result = P::InspectType>],
        clickhouse: &'static CH,
        parser: &'static Parser<T, DB>,
        libmdbx: &'static DB,
        tip_db: &'static DB,
        cli_only: bool,
        metrics: bool,
        is_snapshot: bool,
        cex_window: usize,
        max_pending: usize,
    ) -> Self {
        Self {
            clickhouse,
            range_type,
            min_batch_size,
            max_tasks,
            force_dex_pricing,
            parser,
            libmdbx,
            inspectors,
            quote_asset,
            force_no_dex_pricing,
            cli_only,
            metrics,
            tip_db,
            is_snapshot,
            cex_window,
            max_pending,
            _p: PhantomData,
        }
    }

    pub async fn build(
        self,
        executor: BrontesTaskExecutor,
        shutdown: GracefulShutdown,
    ) -> eyre::Result<Brontes> {
        // we always verify before we allow for any canceling
        if !self.is_snapshot {
            self.verify_global_tables().await?;
        }

        let build_future = self.build_internal(executor.clone());

        pin_mut!(build_future, shutdown);
        tokio::select! {
            res = &mut build_future => {
                return res
            },
            guard = shutdown => {
                drop(guard)
            }
        }
        tracing::info!(
            "Received shutdown signal during initialization process, clearing possibly corrupted
                           full range tables"
        );

        Err(eyre::eyre!("shutdown"))
    }

    async fn build_internal(self, executor: BrontesTaskExecutor) -> eyre::Result<Brontes> {
        let futures = FuturesUnordered::new();

        let pricing_metrics = self.metrics.then(DexPricingMetrics::default);
        let (should_run_tip_inspector, end_block) = self.should_run_tip_inspector().await;

        if self.is_snapshot {
            let (start_block, db_end_block) = self.libmdbx.get_db_range()?;
            if should_run_tip_inspector
                || self.range_type.get_start_block(self.libmdbx) < Some(start_block)
                || end_block > db_end_block
            {
                eyre::bail!(
                    "the current db snapshot block range is {}-{}, please make sure that your set \
                     range falls within these bounds",
                    start_block,
                    db_end_block
                );
            }
        }

        if !should_run_tip_inspector {
            self.build_range_executors(executor.clone(), end_block, pricing_metrics.clone())
                .for_each(|block_range| {
                    futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                        "Range Executor",
                        |shutdown| async move {
                            block_range.run_until_graceful_shutdown(shutdown).await;
                        },
                    ));
                    std::future::ready(())
                })
                .await;
        } else {
            if self.range_type.get_start_block(self.libmdbx).is_some() {
                self.build_range_executors(executor.clone(), end_block, pricing_metrics.clone())
                    .for_each(|block_range| {
                        futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                            "Range Executor",
                            |shutdown| async move {
                                block_range.run_until_graceful_shutdown(shutdown).await;
                            },
                        ));
                        std::future::ready(())
                    })
                    .await;
            }
            tracing::info!("starting tip inspector");
            let tip_inspector = self.build_tip_inspector(
                usize::MAX,
                executor.clone(),
                end_block,
                self.range_type.back_from_tip(),
                pricing_metrics,
            );

            futures.push(executor.spawn_critical_with_graceful_shutdown_signal(
                "Tip Inspector",
                |shutdown| async move { tip_inspector.run_until_graceful_shutdown(shutdown).await },
            ));
        }

        let metrics = FinishedRange::default();
        metrics.running_ranges.increment(futures.len() as f64);
        metrics.total_set_range.increment(
            end_block
                - self
                    .range_type
                    .get_start_block(self.libmdbx)
                    .unwrap_or(end_block),
        );

        Ok(Brontes { futures, metrics })
    }

    //TODO: We currently don't have the ability to stream the query results from
    //TODO: clickhouse because the client is shit, so we have to break up the
    //TODO: downloads into smaller batches & wait for these smaller queries to
    //TODO: return to write all of the data. This uses a lot of memory & is slow.
    //TODO: We will switch to using stream functionality.

    /// Builds a stream of RangeExecutors for processing blocks.
    ///
    /// This function creates a stream of RangeExecutors, each responsible for
    /// processing a chunk of blocks. It handles the initialization of necessary
    /// components and sets up progress tracking.
    ///
    /// # Arguments
    ///
    /// * `executor` - The task executor for spawning asynchronous tasks.
    /// * `end_block` - The final block to process.
    /// * `pricing_metrics` - Optional metrics for DEX pricing.
    ///
    /// # Returns
    ///
    /// Returns a `Stream` that yields `RangeExecutorWithPricing<T, DB, CH, P>`
    /// instances.
    ///
    /// # Notes
    ///
    /// This function uses `buffer_unordered(8)` to limit concurrent
    /// initialization of RangeExecutors. The actual number of concurrently
    /// running executors is not limited by this buffer.
    fn build_range_executors(
        &'_ self,
        executor: BrontesTaskExecutor,
        end_block: u64,
        pricing_metrics: Option<DexPricingMetrics>,
    ) -> impl Stream<Item = RangeExecutorWithPricing<T, DB, CH, P>> + '_ {
        let chunks = match &self.range_type {
            RangeType::SingleRange { start_block, from_db_tip, .. } => {
                let start_block = if *from_db_tip {
                    self.libmdbx
                        .get_most_recent_block()
                        .unwrap_or_else(|_| start_block.unwrap())
                } else {
                    start_block.unwrap()
                };

                self.calculate_chunks(start_block, end_block)
            }
            RangeType::MultipleRanges(ranges) => ranges.clone(),
        };

        let progress_bar = self.initialize_global_progress_bar();

        let state_to_init = Arc::new(self.state_to_initialize(end_block));

        #[cfg(feature = "sorella-server")]
        let mut buffer_size = calculate_buffer_size(&state_to_init, self.max_tasks as usize);

        #[cfg(not(feature = "sorella-server"))]
        let mut buffer_size = 8;

        let mut tables_pb = Arc::new(Vec::new());

        let multi = MultiProgress::default();
        if !self.is_snapshot {
            tables_pb = Arc::new(
                state_to_init
                    .tables_with_init_count()
                    .map(|(table, count)| {
                        (table, table.build_init_state_progress_bar(&multi, count as u64))
                    })
                    .collect_vec(),
            );
        } else {
            buffer_size = (self.max_tasks as usize / 4).clamp(2, 25)
        }

        let range_metrics = self.metrics.then(|| {
            GlobalRangeMetrics::new(chunks.iter().map(|(start, end)| end - start).collect_vec())
        });

        futures::stream::iter(chunks.into_iter().enumerate().map(
            move |(batch_id, (start_block, end_block))| {
                let ranges =
                    state_to_init.get_state_for_ranges(start_block as usize, end_block as usize);

                let executor = executor.clone();
                let prgrs_bar = progress_bar.clone();
                let tables_pb = tables_pb.clone();
                let metrics = range_metrics.clone();
                let pricing_metrics = pricing_metrics.clone();

                #[allow(clippy::async_yields_async)]
                async move {
                    tracing::info!(
                        "Starting batch {batch_id} for block range {start_block}-{end_block}"
                    );

                    if !self.is_snapshot {
                        self.init_block_range_tables(ranges, tables_pb.clone(), self.metrics)
                            .await
                            .unwrap();
                    }

                    #[allow(clippy::async_yields_async)]
                    RangeExecutorWithPricing::new(
                        batch_id,
                        start_block,
                        end_block,
                        self.init_state_collector(
                            batch_id,
                            executor.clone(),
                            start_block,
                            end_block,
                            false,
                            pricing_metrics,
                        ),
                        self.libmdbx,
                        self.inspectors,
                        prgrs_bar,
                        metrics,
                    )
                }
            },
        ))
        .buffer_unordered(buffer_size)
    }

    fn build_tip_inspector(
        &self,
        range_id: usize,
        executor: BrontesTaskExecutor,
        start_block: u64,
        back_from_tip: u64,
        pricing_metrics: Option<DexPricingMetrics>,
    ) -> TipInspector<T, DB, CH, P> {
        let range_metrics = self.metrics.then(|| GlobalRangeMetrics::new(vec![0]));
        let state_collector = self.init_state_collector(
            range_id,
            executor,
            start_block,
            start_block,
            true,
            pricing_metrics,
        );
        TipInspector::new(
            start_block,
            back_from_tip,
            state_collector,
            self.parser,
            self.tip_db,
            self.inspectors,
            range_metrics,
        )
    }

    /// Initializes a StateCollector for a specific range of blocks.
    ///
    /// This function sets up the necessary components for collecting state data
    /// over a range of blocks, including classification, pricing, and metadata
    /// fetching.
    ///
    /// # Arguments
    ///
    /// * `range_id` - A unique identifier for this range.
    /// * `executor` - The task executor for spawning asynchronous tasks.
    /// * `start_block` - The first block in the range.
    /// * `end_block` - The last block in the range.
    /// * `tip` - Boolean flag indicating if this is for tip processing.
    /// * `pricing_metrics` - Optional metrics for DEX pricing.
    ///
    /// # Returns
    ///
    /// Returns a `StateCollector<T, DB, CH>` initialized with the specified
    /// parameters.
    fn init_state_collector(
        &self,
        range_id: usize,
        executor: BrontesTaskExecutor,
        start_block: u64,
        end_block: u64,
        tip: bool,
        pricing_metrics: Option<DexPricingMetrics>,
    ) -> StateCollector<T, DB, CH> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded_channel();
        let classifier = static_object(Classifier::new(self.libmdbx, tx, self.parser.get_tracer()));

        let pairs = self.libmdbx.protocols_created_before(start_block).unwrap();

        let rest_pairs = self
            .libmdbx
            .protocols_created_range(start_block + 1, end_block)
            .unwrap()
            .into_iter()
            .flat_map(|(_, pools)| {
                pools
                    .into_iter()
                    .filter(|(_, p, _)| p.has_state_updater())
                    .map(|(addr, protocol, pair)| (addr, (protocol, pair)))
                    .collect::<Vec<_>>()
            })
            .collect::<FastHashMap<_, _>>();

        let pair_graph = GraphManager::init_from_db_state(pairs, pricing_metrics.clone());

        let data_req = Arc::new(AtomicBool::new(true));

        let pricer = BrontesBatchPricer::new(
            range_id,
            shutdown.clone(),
            self.quote_asset,
            pair_graph,
            UnboundedYapperReceiver::new(rx, 100_000, "batch pricer".into()),
            self.parser.get_tracer(),
            start_block,
            rest_pairs,
            data_req.clone(),
            pricing_metrics.clone(),
            executor.clone(),
            self.max_pending,
        );

        let express_lane_auction_provider = ExpressLaneAuctionProvider::new(self.parser.get_tracer());
        let pricing = WaitingForPricerFuture::new(pricer, executor);
        let fetcher = MetadataLoader::new(
            tip.then_some(self.clickhouse),
            pricing,
            self.force_dex_pricing,
            self.force_no_dex_pricing,
            data_req,
            self.cex_window,
            self.max_pending,
            express_lane_auction_provider,
        );

        let block_window_size = self
            .inspectors
            .iter()
            .max_by_key(|i| i.block_window())
            .map(|v| v.block_window())
            .expect("no inspectors loaded");

        let window = MultiBlockWindow::new(block_window_size);

        StateCollector::new(
            shutdown,
            fetcher,
            classifier,
            self.parser,
            self.libmdbx,
            window,
            self.quote_asset,
        )
    }

    async fn init_block_range_tables(
        &self,
        ranges: Vec<(Tables, Vec<RangeInclusive<u64>>)>,
        tables_pb: Arc<Vec<(Tables, ProgressBar)>>,
        metrics: bool,
    ) -> eyre::Result<()> {
        tracing::info!(?ranges, "initting ranges");
        join_all(ranges.into_iter().flat_map(|(table, ranges)| {
            let tables_pb = tables_pb.clone();
            let mut futs: Vec<Pin<Box<dyn Future<Output = eyre::Result<()>> + Send>>> =
                Vec::with_capacity(ranges.len());

            for range in ranges {
                let start = *range.start();
                let end = *range.end();
                let tables_pb = tables_pb.clone();
                if end - start > 1000 {
                    futs.push(Box::pin(async move {
                        self.libmdbx
                            .initialize_table(
                                self.clickhouse,
                                self.parser.get_tracer(),
                                table,
                                false,
                                Some((start, end)),
                                tables_pb.clone(),
                                metrics,
                            )
                            .await
                    }));
                } else {
                    futs.push(Box::pin(async move {
                        self.libmdbx
                            .initialize_table_arbitrary(
                                self.clickhouse,
                                self.parser.get_tracer(),
                                table,
                                range.collect_vec(),
                                tables_pb.clone(),
                                metrics,
                            )
                            .await
                    }));
                }
            }
            futs
        }))
        .await
        .into_iter()
        .collect::<eyre::Result<Vec<()>>>()?;

        Ok(())
    }

    /// Verify global tables & initialize them if necessary
    async fn verify_global_tables(&self) -> eyre::Result<()> {
        tracing::info!("Initializing critical range state");
        self.libmdbx
            .initialize_full_range_tables(self.clickhouse, self.parser.get_tracer(), true)
            .await?;

        Ok(())
    }

    ///Calculate the block chunks using min batch size and max_tasks.
    /// Max tasks defaults to 50% of physical cores of the system if not set
    fn calculate_chunks(&self, start_block: u64, end_block: u64) -> Vec<(u64, u64)> {
        let range = end_block - start_block;
        let cpus_min = range / self.min_batch_size + 1;
        let cpus = std::cmp::min(cpus_min, self.max_tasks);

        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        (start_block..=end_block)
            .chunks(chunk_size.try_into().unwrap())
            .into_iter()
            .map(|mut c| {
                let start = c.next().unwrap();
                let end_block = c.last().unwrap_or(start_block);
                (start, end_block)
            })
            .collect_vec()
    }

    async fn should_run_tip_inspector(&self) -> (bool, u64) {
        match &self.range_type {
            RangeType::MultipleRanges(ranges) => {
                (false, ranges.last().expect("Specified Range is empty").1)
            }
            RangeType::SingleRange { end_block, back_from_tip, .. } => {
                if let Some(end_block) = end_block {
                    (false, *end_block)
                } else {
                    #[cfg(feature = "local-reth")]
                    let chain_tip = self.parser.get_latest_block_number().unwrap();
                    #[cfg(not(feature = "local-reth"))]
                    let chain_tip = self.parser.get_latest_block_number().await.unwrap();
                    (true, chain_tip - back_from_tip)
                }
            }
        }
    }

    fn state_to_initialize(&self, end: u64) -> StateToInitialize {
        match &self.range_type {
            RangeType::SingleRange { start_block, from_db_tip, .. } => {
                let start_block = if *from_db_tip {
                    self.libmdbx.get_most_recent_block().unwrap()
                } else {
                    start_block.unwrap()
                };
                self.libmdbx.state_to_initialize(start_block, end).unwrap()
            }
            RangeType::MultipleRanges(ranges) => {
                let mut state_to_init = StateToInitialize::default();
                for (start_block, end_block) in ranges {
                    state_to_init.merge(
                        self.libmdbx
                            .state_to_initialize(*start_block, *end_block)
                            .unwrap(),
                    )
                }
                state_to_init
            }
        }
    }

    fn initialize_global_progress_bar(&self) -> Option<ProgressBar> {
        self.cli_only
            .then(|| {
                let total_blocks = match &self.range_type {
                    RangeType::SingleRange { start_block, end_block, .. } => {
                        let start = start_block.as_ref().copied()?;
                        let end = end_block.as_ref().copied()?;
                        end.saturating_sub(start).saturating_add(1)
                    }
                    RangeType::MultipleRanges(ranges) => ranges
                        .iter()
                        .map(|(start, end)| end.saturating_sub(*start).saturating_add(1))
                        .sum(),
                };

                let progress_bar = ProgressBar::with_draw_target(
                    Some(total_blocks),
                    ProgressDrawTarget::stderr_with_hz(100),
                );

                let style = ProgressStyle::default_bar()
                    .template(
                        "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} blocks \
                         ({percent}%) | ETA: {eta}",
                    )
                    .expect("Invalid progress bar template")
                    .progress_chars("█>-")
                    .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                        write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
                    })
                    .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                        write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
                    });

                progress_bar.set_style(style);
                progress_bar.set_message("Processing blocks:");
                Some(progress_bar)
            })
            .flatten()
    }
}

#[cfg(feature = "sorella-server")]
fn calculate_buffer_size(state_to_init: &StateToInitialize, max_tasks: usize) -> usize {
    if state_to_init.ranges_to_init.is_empty() {
        return (max_tasks / 4).clamp(4, 30)
    }

    let initializing_cex = state_to_init.ranges_to_init.contains_key(&Tables::CexPrice)
        || state_to_init
            .ranges_to_init
            .contains_key(&Tables::CexTrades);

    if initializing_cex {
        4
    } else {
        (max_tasks / 10).clamp(4, 15)
    }
}

pub struct Brontes {
    pub futures: FuturesUnordered<JoinHandle<()>>,
    pub metrics: FinishedRange,
}

impl Future for Brontes {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while match self.futures.poll_next_unpin(cx) {
            Poll::Ready(Some(_)) => {
                tracing::info!("range finished");
                self.metrics.running_ranges.decrement(1.0);
                true
            }
            Poll::Ready(None) => return Poll::Ready(()),
            Poll::Pending => return Poll::Pending,
        } {}

        Poll::Pending
    }
}

pub enum RangeType {
    SingleRange {
        start_block:   Option<u64>,
        end_block:     Option<u64>,
        back_from_tip: u64,
        from_db_tip:   bool,
    },
    MultipleRanges(Vec<(u64, u64)>),
}

impl RangeType {
    fn get_start_block<DB: LibmdbxReader>(&self, db: &DB) -> Option<u64> {
        match self {
            RangeType::SingleRange { start_block, from_db_tip, .. } => {
                if !from_db_tip {
                    return *start_block
                }
                db.get_most_recent_block().ok()
            }
            RangeType::MultipleRanges(ranges) => Some(ranges.first().unwrap().0),
        }
    }

    fn back_from_tip(&self) -> u64 {
        match self {
            RangeType::SingleRange { back_from_tip, .. } => *back_from_tip,
            RangeType::MultipleRanges(_) => {
                panic!("Should never be called for specified multi range")
            }
        }
    }
}
