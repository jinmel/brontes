CREATE TABLE IF NOT EXISTS ethereum.relay_payloads (
    `block_number` UInt64,
    `block_hash` String,
    `proposer_fee_recipient` String,
    `value` UInt128
) ENGINE = MergeTree()
PARTITION BY intDiv(block_number, 100000)
ORDER BY (block_number, block_hash)

