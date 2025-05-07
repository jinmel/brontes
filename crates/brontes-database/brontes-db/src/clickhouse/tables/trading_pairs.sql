CREATE TABLE cex.trading_pairs 
(
    `exchange` String,               -- Exchange name
    `pair` String,                   -- Trading pair identifier
    `base_asset` String,             -- Base asset symbol
    `quote_asset` String,            -- Quote asset symbol
    `trading_type` String            -- Type of trading (SPOT/FUTURES)
)
ENGINE = MergeTree()
PRIMARY KEY (exchange, pair)
ORDER BY (exchange, pair);