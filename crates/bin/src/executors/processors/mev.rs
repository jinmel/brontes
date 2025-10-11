#[cfg(feature = "local-clickhouse")]
use std::sync::Arc;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::{
    composer::{run_block_inspection, ComposerResults},
    Inspector,
};
#[cfg(feature = "local-clickhouse")]
use brontes_types::frontend_prunes::{
    remove_burn_transfers, remove_collect_transfers, remove_mint_transfers, remove_swap_transfers,
};
use brontes_types::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::{
        block_analysis::BlockAnalysis,
        dex::{BlockPrice, DexVolume},
        metadata::Metadata,
    },
    execute_on,
    mev::{Bundle, MevBlock, MevType},
    normalized_actions::{Action, NormalizedSwapWithFee},
    pair::Pair,
    tree::{BlockTree, TreeSearchBuilder},
    utils::ToFloatNearest,
    BlockData, FastHashMap, MultiBlockData, Protocol,
};
use tracing::debug;

use crate::Processor;

#[derive(Debug, Clone, Copy)]
pub struct MevProcessor;

impl Processor for MevProcessor {
    type InspectType = Vec<Bundle>;

    #[allow(unused_variables)]
    async fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &'static DB,
        inspectors: &'static [&dyn Inspector<Result = Self::InspectType>],
        data: MultiBlockData,
    ) {
        let last = data.get_most_recent_block().clone();
        let BlockData { metadata, tree } = last;
        if let Err(e) = db
            .write_dex_quotes(metadata.block_num, metadata.dex_quotes.clone())
            .await
        {
            tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex pricing and state into db");
        }

        let volumes = calculate_dex_volumes(&tree, &metadata);
        if let Err(e) = db.write_dex_volumes(volumes).await {
            tracing::error!(err=%e, block_num=metadata.block_num, "failed to insert dex volumes into db");
        }

        #[cfg(feature = "local-clickhouse")]
        {
            let inner_tree = Arc::unwrap_or_clone(tree.clone());
            insert_tree(db, inner_tree, metadata.block_num).await;
        }

        let ComposerResults { block_details, mev_details, block_analysis, .. } =
            execute_on!(async_inspect, { run_block_inspection(inspectors, data, db) }).await;

        insert_mev_results(db, block_details, mev_details, block_analysis).await;
    }
}

#[cfg(feature = "local-clickhouse")]
async fn insert_tree<DB: DBWriter + LibmdbxReader>(
    db: &DB,
    mut tree_owned: BlockTree<Action>,
    block_num: u64,
) {
    remove_swap_transfers(&mut tree_owned);
    remove_mint_transfers(&mut tree_owned);
    remove_burn_transfers(&mut tree_owned);
    remove_collect_transfers(&mut tree_owned);

    if let Err(e) = db.insert_tree(tree_owned).await {
        tracing::error!(err=%e, %block_num, "failed to insert tree into db");
    }
}

fn calculate_dex_volumes(tree: &BlockTree<Action>, meta: &Metadata) -> Vec<DexVolume> {
    let Some(quotes) = meta.dex_quotes.as_ref() else { return Vec::new() };
    let mut volumes: FastHashMap<Protocol, f64> = FastHashMap::default();
    let eth_price = meta.get_eth_price(USDT_ADDRESS);

    for (_tx, actions) in tree
        .clone()
        .collect_all(TreeSearchBuilder::default().with_action(Action::is_swap))
    {
        for action in actions {
            let swap = match action {
                Action::Swap(s) => s,
                Action::SwapWithFee(NormalizedSwapWithFee { swap, .. }) => swap,
                _ => continue,
            };

            if let Some(price_in_weth) = quotes
                .price_for_block(Pair(swap.token_in.address, WETH_ADDRESS), BlockPrice::Average)
            {
                let price_usd = price_in_weth * eth_price.clone();
                let volume = (swap.amount_in.clone() * price_usd).to_float();
                *volumes.entry(swap.protocol).or_default() += volume;
            }
        }
    }

    volumes
        .into_iter()
        .map(|(protocol, volume_usd)| DexVolume {
            block_number: meta.block_num(),
            protocol,
            volume_usd,
        })
        .collect()
}

async fn insert_mev_results<DB: DBWriter + LibmdbxReader>(
    database: &'static DB,
    block_details: MevBlock,
    mev_details: Vec<Bundle>,
    analysis: BlockAnalysis,
) {
    debug!(
        target: "brontes::results",
        "block details\n {}",
        block_details.to_string()
    );

    let block_number = block_details.block_number;
    output_mev_and_update_searcher_info(database, &mev_details).await;

    // Attempt to save the MEV block details
    if let Err(e) = database
        .save_mev_blocks(block_details.block_number, block_details, mev_details)
        .await
    {
        tracing::error!(
            "Failed to insert classified data into libmdbx: {:?} at block: {}",
            e,
            block_number
        );
    }
    if let Err(e) = database.write_block_analysis(analysis).await {
        tracing::error!(
            "Failed to insert block analysis data into db: {:?} at block: {}",
            e,
            block_number
        );
    }
}
async fn output_mev_and_update_searcher_info<DB: DBWriter + LibmdbxReader>(
    database: &DB,
    mev_details: &Vec<Bundle>,
) {
    for mev in mev_details {
        debug!(
            target: "brontes::results",
            "mev details\n {}",
            mev.to_string()
        );

        if mev.header.mev_type == MevType::Unknown || mev.header.mev_type == MevType::SearcherTx {
            continue
        }

        let (eoa_info, contract_info) = database
            .try_fetch_searcher_info(mev.header.eoa, mev.header.mev_contract)
            .expect("Failed to fetch searcher info from the database");

        let mut eoa_info = eoa_info.unwrap_or_default();
        let mut contract_info = contract_info.unwrap_or_default();

        eoa_info.update_with_bundle(&mev.header);
        contract_info.update_with_bundle(&mev.header);

        if let Err(e) = database
            .write_searcher_info(
                mev.header.eoa,
                mev.header.mev_contract,
                eoa_info,
                Some(contract_info),
            )
            .await
        {
            tracing::error!("Failed to update searcher info in the database: {:?}", e);
        }
    }
}
