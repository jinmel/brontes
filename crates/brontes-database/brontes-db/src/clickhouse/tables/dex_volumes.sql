CREATE TABLE IF NOT EXISTS dex.dex_volumes (
    `period` DateTime64(3, 'UTC'),
    `project` String,
    `volume_usd` Float64,
    `recipient` UInt64
) ENGINE = ReplacingMergeTree()
ORDER BY (`period`, `project`)
