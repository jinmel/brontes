use std::str::FromStr;

use serde::{Deserialize, Serialize};
use eyre::WrapErr;
use alloy_chains::NamedChain;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChainConfig {
    pub chain: NamedChain,
}

impl ChainConfig {
    pub fn from_chain_id(chain_id: u64) -> eyre::Result<Self> {
        let chain = NamedChain::try_from(chain_id).wrap_err("Invalid chain id")?;
        Ok(Self { chain })
    }

    pub fn from_chain_name(chain_name: &str) -> eyre::Result<Self> {
        let chain = NamedChain::from_str(chain_name).wrap_err("Invalid chain name")?;
        Ok(Self { chain })
    }

    pub fn get_chain_id(&self) -> u64 {
        self.chain.into()
    }

    pub fn get_chain_name(&self) -> String {
        self.chain.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_config() {
        // Test from_chain_id
        let config = ChainConfig::from_chain_id(42161).unwrap();
        assert_eq!(config.chain, NamedChain::Arbitrum);
        assert_eq!(config.get_chain_id(), 42161);
        assert_eq!(config.get_chain_name(), "arbitrum");

        // Test from_chain_name
        let config = ChainConfig::from_chain_name("arbitrum").unwrap();
        assert_eq!(config.chain, NamedChain::Arbitrum);
        assert_eq!(config.get_chain_id(), 42161);
        assert_eq!(config.get_chain_name(), "arbitrum");

        // Test mainnet
        let config = ChainConfig::from_chain_id(1).unwrap();
        assert_eq!(config.chain, NamedChain::Mainnet);
        assert_eq!(config.get_chain_id(), 1);
        assert_eq!(config.get_chain_name(), "mainnet");

        let config = ChainConfig::from_chain_name("mainnet").unwrap();
        assert_eq!(config.chain, NamedChain::Mainnet);
        assert_eq!(config.get_chain_id(), 1);
        assert_eq!(config.get_chain_name(), "mainnet");

        // Test error cases
        assert!(ChainConfig::from_chain_id(999999).is_err());
        assert!(ChainConfig::from_chain_name("invalid-chain").is_err());
    }
}
