#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};

use alloy_primitives::{Address, Bytes};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_metrics::classifier::ClassificationMetrics;
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::pool::NormalizedNewPool, structured_trace::CallFrameInfo,
    traits::TracingProvider,
};
use futures::Future;

pub mod tree_builder;
pub use tree_builder::Classifier;
pub mod discovery_logs_only;
pub mod discovery_only;
pub mod multi_frame_classification;

#[cfg(feature = "tests")]
pub mod test_utils;

mod classifiers;
use alloy_sol_types::sol;
use brontes_types::normalized_actions::Action;
pub use classifiers::*;

// Actions
sol!(UniswapV2, "./classifier-abis/UniswapV2.json");
sol!(UniswapV4, "./classifier-abis/UniswapV4.json");

sol!(SushiSwapV2, "./classifier-abis/SushiSwapV2.json");
sol!(UniswapV3, "./classifier-abis/UniswapV3.json");
sol!(SushiSwapV3, "./classifier-abis/SushiSwapV3.json");
sol!(PancakeSwapV2, "./classifier-abis/PancakeSwapV2.json");
sol!(PancakeSwapV3, "./classifier-abis/PancakeSwapV3.json");
sol!(CurveBase2, "./classifier-abis/CurveBase2.json");
//sol!(CurveLido2, "./classifier-abis/CurveBase2Lido.json");
sol!(CurveBase3, "./classifier-abis/CurveBase3.json");
sol!(CurveBase4, "./classifier-abis/CurveBase4.json");
sol!(CurveV1MetapoolImpl, "./classifier-abis/CurveV1MetapoolImpl.json");
sol!(CurveV2MetapoolImpl, "./classifier-abis/CurveV2MetapoolImpl.json");
sol!(CurveV2PlainImpl, "./classifier-abis/CurveV2PlainImpl.json");
sol!(CurvecrvUSDPlainImpl, "./classifier-abis/CurvecrvUSDPlainImpl.json");
sol!(CurveCryptoSwap, "./classifier-abis/CurveCryptoSwap.json");
sol!(BalancerV1, "./classifier-abis/balancer/BalancerV1Pool.json");
sol!(BalancerV2Vault, "./classifier-abis/balancer/BalancerV2Vault.json");
sol!(AaveV2, "./classifier-abis/AaveV2Pool.json");
sol!(AaveV3, "./classifier-abis/AaveV3Pool.json");
sol!(UniswapX, "./classifier-abis/UniswapXExclusiveDutchOrderReactor.json");
sol!(MakerPSM, "./classifier-abis/maker/MakerPSM.json");
sol!(MakerDssFlash, "./classifier-abis/maker/MakerDssFlash.json");
sol!(CompoundV2CToken, "./classifier-abis/CompoundV2CToken.json");
sol!(Comet, "./classifier-abis/Comet.json");
sol!(OneInchAggregationRouterV5, "./classifier-abis/OneInchAggregationRouterV5.json");
sol!(OneInchFusionSettlement, "./classifier-abis/OneInchFusionSettlement.json");
sol!(ClipperExchange, "./classifier-abis/ClipperExchange.json");
sol!(CowswapGPv2Settlement, "./classifier-abis/cowswap/GPv2Settlement.json");
sol!(ZeroXUniswapFeaure, "./classifier-abis/zero-x/ZeroXUniswapFeature.json");
sol!(ZeroXUniswapV3Feature, "./classifier-abis/zero-x/ZeroXUniswapV3Feature.json");
sol!(ZeroXTransformERC20Feature, "./classifier-abis/zero-x/ZeroXTransformERC20Feature.json");
sol!(ZeroXPancakeSwapFeature, "./classifier-abis/zero-x/ZeroXPancakeSwapFeature.json");
sol!(ZeroXOtcOrdersFeature, "./classifier-abis/zero-x/ZeroXOtcOrdersFeature.json");
sol!(ZeroXLiquidityProviderFeature, "./classifier-abis/zero-x/ZeroXLiquidityProviderFeature.json");
sol!(ZeroXInterface, "./classifier-abis/zero-x/ZeroXInterface.json");
sol!(DodoDPPPool, "./classifier-abis/dodo/DPPPool.json");
sol!(DodoDSPPool, "./classifier-abis/dodo/DSPPool.json");
sol!(MaverickV2Pool, "./classifier-abis/maverick_v2/MaverickV2Pool.json");
sol!(CamelotV2, "./classifier-abis/CamelotV2Pair.json");
sol!(CamelotV3, "./classifier-abis/Algebra1_9Pool.json");
sol!(Dexalot, "./classifier-abis/Dexalot.json");
sol!(LFJPair, "./classifier-abis/LFJ/ILBPair.json");
sol!(LFJV2_2Pair, "./classifier-abis/LFJ/ILBPair.json");
sol!(PendleMarketV3, "./classifier-abis/pendle_v2/PendleMarketV3.json");
sol!(PendlePrincipalToken, "./classifier-abis/pendle_v2/PendlePrincipalToken.json");
sol!(PendleYieldToken, "./classifier-abis/pendle_v2/PendleYieldToken.json");
sol!(PendleSYToken, "./classifier-abis/pendle_v2/IStandardizedYield.json");
sol!(GMXV1, "./classifier-abis/gmx_v1/Vault.json");
sol!(BalancerV3Vault, "./classifier-abis/balancer/BalancerV3Vault.json");
sol!(FluidDexT1, "./classifier-abis/fluid/FluidDexT1.json");
sol!(FluidLending, "./classifier-abis/fluid/FluidLending.json");
sol!(DolomiteLiquidator, "./classifier-abis/dolomite/Liquidator.json");
sol!(FluidVaultResolver, "./classifier-abis/fluid/FluidVaultResolver.json");
sol!(FluidDexResolver, "./classifier-abis/fluid/FluidDexResolver.json");
sol!(FluidSmartLending, "./classifier-abis/fluid/FluidSmartLending.json");
sol!(FluidVault, "./classifier-abis/fluid/FluidVault.json");
// Discovery
sol!(BalancerV3VaultExtension, "./classifier-abis/balancer/BalancerV3VaultExtension.json");
sol!(UniswapV2Factory, "./classifier-abis/UniswapV2Factory.json");
sol!(UniswapV3Factory, "./classifier-abis/UniswapV3Factory.json");
sol!(UniswapV4Factory, "./classifier-abis/UniswapV4.json");
sol!(CurveV1MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV1.json");
sol!(CurveV2MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV2.json");
sol!(CurvecrvUSDFactory, "./classifier-abis/CurveCRVUSDFactory.json");
sol!(CurveCryptoSwapFactory, "./classifier-abis/CurveCryptoSwapFactory.json");
sol!(CurveTriCryptoFactory, "./classifier-abis/CurveTriCryptoFactory.json");
sol!(PancakeSwapV3PoolDeployer, "./classifier-abis/PancakeSwapV3PoolDeployer.json");
sol!(CompoundV2Comptroller, "./classifier-abis/CompoundV2Comptroller.json");
sol!(CometConfigurator, "./classifier-abis/CometConfigurator.json");
sol!(CErc20Delegate, "./classifier-abis/CErc20Delegate.json");
sol!(BalancerV1CorePoolFactory, "./classifier-abis/balancer/BalancerV1Factory.json");
sol!(BalancerV1SmartPoolFactory, "./classifier-abis/balancer/BalancerV1CrpFactory.json");
sol!(DodoDVMFactory, "./classifier-abis/dodo/DVMFactory.json");
sol!(DodoDPPFactory, "./classifier-abis/dodo/DPPFactory.json");
sol!(DodoDSPFactory, "./classifier-abis/dodo/DSPFactory.json");
sol!(MaverickV2Factory, "./classifier-abis/maverick_v2/MaverickV2Factory.json");
sol!(CamelotV2Factory, "./classifier-abis/CamelotV2Factory.json");
sol!(CamelotV3Factory, "./classifier-abis/Algebra1_9Factory.json");
sol!(LFJFactory, "./classifier-abis/LFJ/ILBFactory.json");
sol!(LFJV2_2Factory, "./classifier-abis/LFJ/ILBFactory.json");
sol!(PendleMarketV3Factory, "./classifier-abis/pendle_v2/PendleMarketFactoryV3.json");
sol!(PendleYieldContractFactory, "./classifier-abis/pendle_v2/PendleYieldContractFactory.json");
sol!(FluidDexFactory, "./classifier-abis/fluid/FluidDexFactory.json");
sol!(DolomiteAdmin, "./classifier-abis/dolomite/Admin.json");
sol!(FluidForwarder, "./classifier-abis/fluid/AvocadoForwarder.json");
sol!(FluidVaultFactory, "./classifier-abis/fluid/FluidVaultFactory.json");
// Balancer Pool Interfaces
sol! {
    enum SwapKind {
        GIVEN_IN,
        GIVEN_OUT
    }

    struct SwapRequest {
        SwapKind kind;
        address tokenIn;
        address tokenOut;
        uint256 amount;
        // Misc data
        bytes32 poolId;
        uint256 lastChangeBlock;
        address from;
        address to;
        bytes userData;
    }

    interface IGeneralPool {
        function onSwap(
            SwapRequest memory swapRequest,
            uint256[] memory balances,
            uint256 indexIn,
            uint256 indexOut
        ) external returns (uint256 amount);
    }

    interface IMinimalSwapInfoPool {
        function onSwap(
            SwapRequest memory swapRequest,
            uint256 currentBalanceTokenIn,
            uint256 currentBalanceTokenOut
        ) external returns (uint256 amount);
    }
}

sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    function name() public view returns (string);
    function symbol() public view returns (string);
    function decimals() public view returns (uint8);
    function totalSupply() public view returns (uint256);
}

pub static CLASSIFICATION_METRICS: OnceLock<ClassificationMetrics> = OnceLock::new();

pub trait ActionCollection: Sync + Send {
    fn dispatch<DB: LibmdbxReader + DBWriter, T: TracingProvider>(
        &self,
        call_info: CallFrameInfo<'_>,
        db_tx: &DB,
        block: u64,
        tx_idx: u64,
        tracer: Arc<T>,
    ) -> Option<(DexPriceMsg, Action)>;
}

pub trait IntoAction: Debug + Send + Sync {
    fn decode_call_trace<DB: LibmdbxReader + DBWriter, T: TracingProvider>(
        &self,
        call_info: CallFrameInfo<'_>,
        block: u64,
        tx_idx: u64,
        db_tx: &DB,
        tracer: Arc<T>,
    ) -> eyre::Result<DexPriceMsg>;
}

pub trait FactoryDiscovery {
    fn decode_create_trace<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        deployed_address: Address,
        trace_idx: u64,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<NormalizedNewPool>> + Send;
}

pub trait FactoryDiscoveryDispatch: Sync + Send {
    fn dispatch<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        possible_calls: Vec<(Address, Bytes)>,
        deployed_address: Address,
        trace_idx: u64,
    ) -> impl Future<Output = Vec<NormalizedNewPool>> + Send;
}
