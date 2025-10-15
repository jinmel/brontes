CREATE TABLE IF NOT EXISTS ethereum.block_observations (
    `block_number` UInt64,
    `block_hash` String,
    `timestamp` UInt64
) ENGINE = MergeTree()
PARTITION BY intDiv(block_number, 100000)
ORDER BY (block_number, block_hash, timestamp)

