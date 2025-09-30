CREATE TABLE IF NOT EXISTS dex.dex_volumes (
    `block_number` UInt64,
    `protocol` String,
    `volume_usd` Float64
) ENGINE = MergeTree()
ORDER BY (`block_number`, `protocol`)
