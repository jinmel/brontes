CREATE TABLE IF NOT EXISTS dex.dex_volumes (
    `period` DateTime64(3, 'UTC'),
    `project` String,
    `volume_usd` Nullable(Float64) DEFAULT 0,
    `recipient` Nullable(UInt64) DEFAULT 0
) ENGINE = ReplacingMergeTree()
ORDER BY (`period`, `project`)
