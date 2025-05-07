CREATE TABLE cex.normalized_quotes 
(
    `exchange` String,               -- Exchange name
    `symbol` String,                 -- Trading pair symbol
    `timestamp` UInt64,             -- Microsecond timestamp
    `ask_amount` Float64,           -- Amount available at ask price
    `ask_price` Float64,            -- Ask price
    `bid_price` Float64,            -- Bid price
    `bid_amount` Float64            -- Amount available at bid price
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, exchange, symbol)
ORDER BY (timestamp, exchange, symbol);