CREATE TABLE cex.trading_volume_by_month 
(
    `month` Date,                    -- Start of the month
    `symbol` String,                 -- Trading pair symbol
    `exchange` String,               -- Exchange name
    `sum_volume` Float64            -- Total trading volume for the month
)
ENGINE = MergeTree()
PRIMARY KEY (month, symbol, exchange)
ORDER BY (month, symbol, exchange); 