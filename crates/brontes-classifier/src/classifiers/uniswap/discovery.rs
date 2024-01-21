use alloy_primitives::Address;
use brontes_macros::discovery_impl;
use brontes_pricing::types::DiscoveredPool;
use brontes_types::exchanges::StaticBindingsDb;

discovery_impl!(
    UniswapV2Decoder,
    crate::UniswapV2Factory::createPairCall,
    0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f,
    |deployed_address: Address, call_data: createPairCall| {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![DiscoveredPool::new(
            vec![token_a, token_b],
            deployed_address,
            StaticBindingsDb::UniswapV2,
        )]
    }
);

discovery_impl!(
    UniswapV3Decoder,
    crate::UniswapV3Factory::createPoolCall,
    0x1F98431c8aD98523631AE4a59f267346ea31F984,
    |deployed_address: Address, call_data: createPoolCall| {
        let token_a = call_data.tokenA;
        let token_b = call_data.tokenB;

        vec![DiscoveredPool::new(
            vec![token_a, token_b],
            deployed_address,
            StaticBindingsDb::UniswapV3,
        )]
    }
);
