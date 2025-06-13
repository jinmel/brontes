use std::str::FromStr;

use serde::{Deserialize, Serialize};
use eyre::WrapErr;
use alloy_chains::NamedChain;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChainConfig {
    pub chain: NamedChain,
}

impl ChainConfig {
    pub fn new(chain_id_or_name: String) -> eyre::Result<Self> {
        let chain_id = chain_id_or_name.parse::<u64>().ok();

        let chain = if let Some(chain_id) = chain_id {
            NamedChain::try_from(chain_id).wrap_err("Invalid chain id")?
        } else {
            NamedChain::from_str(&chain_id_or_name).wrap_err("Invalid chain id")?
        };

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
        let config = ChainConfig::new("42161".to_string()).unwrap();
        assert_eq!(config.chain, NamedChain::Arbitrum);
        assert_eq!(config.get_chain_id(), 42161);
        assert_eq!(config.get_chain_name(), "arbitrum");

        let config = ChainConfig::new("arbitrum".to_string()).unwrap();
        assert_eq!(config.chain, NamedChain::Arbitrum);
        assert_eq!(config.get_chain_id(), 42161);
        assert_eq!(config.get_chain_name(), "arbitrum");

        // Test error cases
        assert!(ChainConfig::new("999999".to_string()).is_err());
        assert!(ChainConfig::new("invalid-chain".to_string()).is_err());
    }
}
