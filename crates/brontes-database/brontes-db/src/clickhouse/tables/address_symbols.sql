CREATE TABLE cex.address_symbols 
(
    `address` String,                -- Token contract address
    `symbol` String,                 -- Trading symbol
    `unwrapped_symbol` String        -- Unwrapped version of the symbol (e.g., WBTC -> BTC)
)
ENGINE = MergeTree()
PRIMARY KEY (address, symbol)
ORDER BY (address, symbol);
