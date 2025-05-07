CREATE TABLE ethereum.pools 
(
    `address` String,                -- Pool contract address
    `init_block` UInt64,            -- Block number where pool was initialized
    `tokens` Array(String)          -- Array of token addresses in the pool
)
ENGINE = MergeTree()
PRIMARY KEY (address, init_block)
ORDER BY (address, init_block);
