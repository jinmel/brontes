CREATE TABLE IF NOT EXISTS ethereum.chainbound.block_observations (
    block_number UInt64,
    block_hash String,
    timestamp UInt64
) ENGINE = MergeTree()
ORDER BY (block_number, block_hash); 