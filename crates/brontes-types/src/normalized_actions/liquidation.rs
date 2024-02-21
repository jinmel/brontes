use std::fmt::{self, Debug};

use alloy_primitives::U256;
use clickhouse::Row;
use colored::Colorize;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

pub use super::{Actions, NormalizedSwap};
use crate::{
    db::{
        redefined_types::{malachite::RationalRedefined, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    rational_to_clickhouse_tuple, Protocol,
};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedLiquidation {
    #[redefined(same_fields)]
    pub protocol:              Protocol,
    pub trace_index:           u64,
    pub pool:                  Address,
    pub liquidator:            Address,
    pub debtor:                Address,
    pub collateral_asset:      TokenInfoWithAddress,
    pub debt_asset:            TokenInfoWithAddress,
    pub covered_debt:          Rational,
    pub liquidated_collateral: Rational,
    pub msg_value:             U256,
}

impl fmt::Display for NormalizedLiquidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let protocol = self.protocol.to_string().bold();
        let pool_address = format!("{}", self.pool).cyan();
        let liquidator_address = format!("{}", self.liquidator).cyan();
        let debtor_address = format!("{}", self.debtor).cyan();
        let collateral_asset_symbol = self.collateral_asset.inner.symbol.bold();
        let debt_asset_symbol = self.debt_asset.inner.symbol.bold();
        let covered_debt_formatted = format!("{:.4}", self.covered_debt).green();
        let liquidated_collateral_formatted = format!("{:.4}", self.liquidated_collateral).red();

        write!(
            f,
            "Protocol {} - Pool: {}, Liquidator: {}, Debtor: {}, Collateral: {}, Debt: {}, \
             Covered Debt: {}, Liquidated Collateral: {}",
            protocol,
            pool_address,
            liquidator_address,
            debtor_address,
            collateral_asset_symbol,
            debt_asset_symbol,
            covered_debt_formatted,
            liquidated_collateral_formatted
        )
    }
}

impl NormalizedLiquidation {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        if self.protocol == Protocol::AaveV3 || self.protocol == Protocol::AaveV2 {
            actions
                .into_iter()
                .find_map(|(index, action)| {
                    if let Actions::Transfer(transfer) = action {
                        // because aave has the option to return the Atoken or regular,
                        // we can't filter by collateral filter. This might be an issue...
                        // tbd tho
                        if transfer.to == self.liquidator {
                            self.liquidated_collateral = transfer.amount;
                            return Some(index);
                        }
                    }

                    None
                })
                .map(|e| vec![e])
                .unwrap_or_default()
        } else {
            vec![]
        }
    }

    pub fn pretty_print(&self, f: &mut fmt::Formatter<'_>, spaces: usize) -> fmt::Result {
        let field_names = [
            "Protocol",
            "Pool",
            "Liquidator",
            "Debtor",
            "Collateral",
            "Debt",
            "Covered Debt",
            "Liquidated Collateral",
        ];
        let max_field_name_length = field_names.iter().map(|name| name.len()).max().unwrap_or(0);
        let indent = " ".repeat(spaces);

        let protocol = self.protocol.to_string().bright_yellow();
        let pool_address = format!("{}", self.pool).bright_yellow();
        let liquidator_address = format!("{}", self.liquidator).bright_yellow();
        let debtor_address = format!("{}", self.debtor).bright_yellow();
        let collateral_asset_symbol = self.collateral_asset.inner.symbol.clone().bright_yellow();
        let debt_asset_symbol = self.debt_asset.inner.symbol.clone().bright_yellow();
        let covered_debt_formatted = format!("{:.4}", self.covered_debt).bright_yellow();
        let liquidated_collateral_formatted =
            format!("{:.4}", self.liquidated_collateral).bright_yellow();

        writeln!(
            f,
            "{indent}{:width$}: {}\n{indent}{:width$}: {}\n{indent}{:width$}: \
             {}\n{indent}{:width$}: {}\n{indent}{:width$}: {}\n{indent}{:width$}: \
             {}\n{indent}{:width$}: {}\n{indent}{:width$}: {}",
            "Protocol",
            protocol,
            "Pool",
            pool_address,
            "Liquidator",
            liquidator_address,
            "Debtor",
            debtor_address,
            "Collateral",
            collateral_asset_symbol,
            "Debt",
            debt_asset_symbol,
            "Covered Debt",
            covered_debt_formatted,
            "Liquidated Collateral",
            liquidated_collateral_formatted,
            indent = indent,
            width = max_field_name_length + spaces + 1
        )?;

        Ok(())
    }
}

pub struct ClickhouseVecNormalizedLiquidation {
    pub trace_index:           Vec<u64>,
    pub pool:                  Vec<String>,
    pub liquidator:            Vec<String>,
    pub debtor:                Vec<String>,
    pub collateral_asset:      Vec<String>,
    pub debt_asset:            Vec<String>,
    pub covered_debt:          Vec<([u8; 32], [u8; 32])>,
    pub liquidated_collateral: Vec<([u8; 32], [u8; 32])>,
}

impl From<Vec<NormalizedLiquidation>> for ClickhouseVecNormalizedLiquidation {
    fn from(value: Vec<NormalizedLiquidation>) -> Self {
        ClickhouseVecNormalizedLiquidation {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            liquidator:  value
                .iter()
                .map(|val| format!("{:?}", val.liquidator))
                .collect(),
            debtor:      value
                .iter()
                .map(|val| format!("{:?}", val.debtor))
                .collect(),

            collateral_asset:      value
                .iter()
                .map(|val| format!("{:?}", val.collateral_asset))
                .collect(),
            debt_asset:            value
                .iter()
                .map(|val| format!("{:?}", val.debt_asset))
                .collect(),
            covered_debt:          value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.covered_debt))
                .collect(),
            liquidated_collateral: value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.liquidated_collateral))
                .collect(),
        }
    }
}
