CREATE TABLE IF NOT EXISTS eth_analytics.private_txs (
    `block_number` UInt64,
    `tx_hash` String
) ENGINE = MergeTree()
PARTITION BY intDiv(block_number, 100000)
ORDER BY (block_number, tx_hash)

