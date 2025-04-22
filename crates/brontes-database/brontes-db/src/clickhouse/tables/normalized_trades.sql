CREATE TABLE cex.normalized_trades (
    symbol LowCardinality(String),           -- Trading pair symbol (e.g., "BTC/USDT")
    exchange LowCardinality(String),         -- Exchange name
    side LowCardinality(String),             -- Trade side (e.g., "buy" or "sell")
    timestamp UInt64,        -- Unix timestamp in microseconds
    amount Float64,          -- Trade amount/volume
    price Float64            -- Trade price
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, symbol)
ORDER BY (timestamp, symbol)