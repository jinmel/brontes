CREATE TABLE cex.normalized_trades 
(
    `exchange` String,               -- Exchange name
    `symbol` String,                 -- Trading pair symbol
    `timestamp` UInt64,             -- Microsecond timestamp
    `side` String,                  -- Trade side (buy/sell)
    `price` Float64,                -- Trade price
    `amount` Float64                -- Trade amount/volume
)
ENGINE = MergeTree()
PRIMARY KEY (timestamp, exchange, symbol)
ORDER BY (timestamp, exchange, symbol);