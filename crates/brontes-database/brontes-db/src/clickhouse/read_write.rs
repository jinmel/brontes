use std::sync::Arc;

use alloy_primitives::Address;
use brontes_types::{
    db::{
        address_metadata::AddressMetadata,
        address_to_protocol_info::ProtocolInfo,
        block_analysis::BlockAnalysis,
        builder::BuilderInfo,
        dex::DexQuotes,
        metadata::Metadata,
        mev_block::MevBlockWithClassified,
        searcher::SearcherInfo,
        token_info::TokenInfoWithAddress,
        traits::{DBWriter, LibmdbxReader, ProtocolCreatedRange},
    },
    mev::{Bundle, MevBlock},
    normalized_actions::Action,
    pair::Pair,
    structured_trace::TxTrace,
    traits::TracingProvider,
    BlockTree, FastHashMap, Protocol,
};
use indicatif::ProgressBar;

use super::Clickhouse;
use crate::{
    clickhouse::ClickhouseHandle,
    libmdbx::{LibmdbxInit, StateToInitialize},
    Tables,
};

/// A pure Clickhouse database handle that implements the necessary traits
/// without any libmdbx dependency. This replaces ClickhouseMiddleware for
/// Clickhouse-only deployments.
pub struct ClickhouseReadWriter {
    pub client: Clickhouse,
}

impl ClickhouseReadWriter {
    pub fn new(client: Clickhouse) -> Self {
        Self { client }
    }
}

impl DBWriter for ClickhouseReadWriter {
    type Inner = Self;

    fn inner(&self) -> &Self::Inner {
        self
    }

    async fn write_block_analysis(&self, block_analysis: BlockAnalysis) -> eyre::Result<()> {
        self.client.block_analysis(block_analysis).await
    }

    async fn write_dex_quotes(
        &self,
        block_number: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        self.client.write_dex_quotes(block_number, quotes).await
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.client.write_token_info(address, decimals, symbol).await
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client.save_mev_blocks(block_number, block, mev).await
    }

    async fn write_searcher_eoa_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_eoa_info(searcher_eoa, searcher_info)
            .await
    }

    async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_searcher_contract_info(searcher_contract, searcher_info)
            .await
    }

    async fn write_builder_info(
        &self,
        builder_coinbase_addr: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        self.client
            .write_builder_info(builder_coinbase_addr, builder_info)
            .await
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        self.client
            .insert_pool(block, address, tokens, curve_lp_token, classifier_name)
            .await
    }

    async fn insert_tree(&self, tree: BlockTree<Action>) -> eyre::Result<()> {
        self.client.insert_tree(tree).await
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client.save_traces(block, traces).await
    }
}

impl LibmdbxInit for ClickhouseReadWriter {
    async fn initialize_table<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        _clickhouse: &'static CH,
        _tracer: Arc<T>,
        _tables: Tables,
        _clear_tables: bool,
        _block_range: Option<(u64, u64)>,
        _progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        _metrics: bool,
    ) -> eyre::Result<()> {
        // No-op: Clickhouse doesn't need initialization in the same way as libmdbx
        Ok(())
    }

    fn get_db_range(&self) -> eyre::Result<(u64, u64)> {
        // Return a default range or error - this would need to query Clickhouse
        // for actual block range if needed
        Err(eyre::eyre!(
            "get_db_range not implemented for ClickhouseReadWriter"
        ))
    }

    async fn initialize_table_arbitrary<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        _clickhouse: &'static CH,
        _tracer: Arc<T>,
        _tables: Tables,
        _block_range: Vec<u64>,
        _progress_bar: Arc<Vec<(Tables, ProgressBar)>>,
        _metrics: bool,
    ) -> eyre::Result<()> {
        // No-op: Clickhouse doesn't need initialization in the same way as libmdbx
        Ok(())
    }

    async fn initialize_full_range_tables<T: TracingProvider, CH: ClickhouseHandle>(
        &'static self,
        _clickhouse: &'static CH,
        _tracer: Arc<T>,
        _metrics: bool,
    ) -> eyre::Result<()> {
        // No-op: Clickhouse doesn't need initialization in the same way as libmdbx
        Ok(())
    }

    fn state_to_initialize(
        &self,
        _start_block: u64,
        _end_block: u64,
    ) -> eyre::Result<StateToInitialize> {
        // Return empty state - Clickhouse doesn't track initialization state
        Ok(StateToInitialize::default())
    }
}

impl LibmdbxReader for ClickhouseReadWriter {
    fn get_most_recent_block(&self) -> eyre::Result<u64> {
        self.client.get_most_recent_block()
    }

    fn has_dex_quotes(&self, _block_num: u64) -> eyre::Result<bool> {
        // Could be implemented with a Clickhouse query to check existence
        Err(eyre::eyre!(
            "has_dex_quotes not implemented for ClickhouseReadWriter"
        ))
    }

    fn get_cex_trades(
        &self,
        _block: u64,
    ) -> eyre::Result<brontes_types::db::cex::trades::CexTradeMap> {
        // This would need async implementation - not suitable for sync trait
        Err(eyre::eyre!(
            "get_cex_trades not implemented for ClickhouseReadWriter - use async methods instead"
        ))
    }

    fn get_metadata_no_dex_price(
        &self,
        _block_num: u64,
        _quote_asset: Address,
    ) -> eyre::Result<Metadata> {
        // This would need async implementation - not suitable for sync trait
        Err(eyre::eyre!(
            "get_metadata_no_dex_price not implemented for ClickhouseReadWriter - use async \
             methods instead"
        ))
    }

    fn try_fetch_searcher_eoa_info(
        &self,
        _searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(None)
    }

    fn try_fetch_searcher_eoa_infos(
        &self,
        _searcher_eoa: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(FastHashMap::default())
    }

    fn try_fetch_searcher_contract_infos(
        &self,
        _searcher_eoa: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, SearcherInfo>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(FastHashMap::default())
    }

    fn try_fetch_searcher_contract_info(
        &self,
        _searcher_eoa: Address,
    ) -> eyre::Result<Option<SearcherInfo>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(None)
    }

    fn fetch_all_searcher_eoa_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(Vec::new())
    }

    fn fetch_all_searcher_contract_info(&self) -> eyre::Result<Vec<(Address, SearcherInfo)>> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok(Vec::new())
    }

    fn fetch_all_searcher_info(
        &self,
    ) -> eyre::Result<(Vec<(Address, SearcherInfo)>, Vec<(Address, SearcherInfo)>)> {
        // Searcher info is not stored in Clickhouse in the current implementation
        Ok((Vec::new(), Vec::new()))
    }

    fn try_fetch_builder_info(
        &self,
        _builder_coinbase_addr: Address,
    ) -> eyre::Result<Option<BuilderInfo>> {
        // Builder info is not stored in Clickhouse in the current implementation
        Ok(None)
    }

    fn fetch_all_builder_info(&self) -> eyre::Result<Vec<(Address, BuilderInfo)>> {
        // Builder info is not stored in Clickhouse in the current implementation
        Ok(Vec::new())
    }

    fn try_fetch_mev_blocks(
        &self,
        _start_block: Option<u64>,
        _end_block: u64,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "try_fetch_mev_blocks not implemented for ClickhouseReadWriter"
        ))
    }

    fn fetch_all_mev_blocks(
        &self,
        _start_block: Option<u64>,
    ) -> eyre::Result<Vec<MevBlockWithClassified>> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "fetch_all_mev_blocks not implemented for ClickhouseReadWriter"
        ))
    }

    fn get_metadata(&self, _block_num: u64, _quote_asset: Address) -> eyre::Result<Metadata> {
        // This would need async implementation - not suitable for sync trait
        Err(eyre::eyre!(
            "get_metadata not implemented for ClickhouseReadWriter - use async methods instead"
        ))
    }

    fn try_fetch_address_metadata(
        &self,
        _address: Address,
    ) -> eyre::Result<Option<AddressMetadata>> {
        // Address metadata is not stored in Clickhouse in the current implementation
        Ok(None)
    }

    fn try_fetch_address_metadatas(
        &self,
        _address: Vec<Address>,
    ) -> eyre::Result<FastHashMap<Address, AddressMetadata>> {
        // Address metadata is not stored in Clickhouse in the current implementation
        Ok(FastHashMap::default())
    }

    fn fetch_all_address_metadata(&self) -> eyre::Result<Vec<(Address, AddressMetadata)>> {
        // Address metadata is not stored in Clickhouse in the current implementation
        Ok(Vec::new())
    }

    fn get_dex_quotes(&self, _block: u64) -> eyre::Result<DexQuotes> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "get_dex_quotes not implemented for ClickhouseReadWriter"
        ))
    }

    fn try_fetch_token_info(&self, _address: Address) -> eyre::Result<TokenInfoWithAddress> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "try_fetch_token_info not implemented for ClickhouseReadWriter"
        ))
    }

    fn protocols_created_before(
        &self,
        _start_block: u64,
    ) -> eyre::Result<FastHashMap<(Address, Protocol), Pair>> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "protocols_created_before not implemented for ClickhouseReadWriter"
        ))
    }

    fn protocols_created_range(
        &self,
        _start_block: u64,
        _end_block: u64,
    ) -> eyre::Result<ProtocolCreatedRange> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "protocols_created_range not implemented for ClickhouseReadWriter"
        ))
    }

    fn get_protocol_details(&self, _address: Address) -> eyre::Result<ProtocolInfo> {
        // This would need to be implemented with Clickhouse queries
        Err(eyre::eyre!(
            "get_protocol_details not implemented for ClickhouseReadWriter"
        ))
    }

    fn load_trace(&self, _block_num: u64) -> eyre::Result<Vec<TxTrace>> {
        self.client.load_trace(_block_num)
    }
}

