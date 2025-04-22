CREATE TABLE cex.normalized_quotes (
    exchange LowCardinality(String),           -- The exchange name
    symbol LowCardinality(String),             -- The trading pair symbol (e.g., BTC/USD)
    timestamp UInt64,           -- Microsecond timestamp
    ask_amount Float64,        -- Amount available at ask price
    ask_price Float64,         -- Ask price
    bid_price Float64,         -- Bid price
    bid_amount Float64         -- Amount available at bid price
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, symbol)
ORDER BY (timestamp, symbol)