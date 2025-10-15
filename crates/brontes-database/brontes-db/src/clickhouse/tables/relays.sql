CREATE TABLE IF NOT EXISTS ethereum.relays (
    `block_number` UInt64,
    `block_hash` String,
    `ultrasound_adj_block_hash` Nullable(String),
    `timestamp` UInt64,
    `proposer_fee_recipient` String,
    `value` UInt128,
    `ultrasound_adj_value` Nullable(UInt128)
) ENGINE = MergeTree()
PARTITION BY intDiv(block_number, 100000)
ORDER BY (block_number, block_hash)

