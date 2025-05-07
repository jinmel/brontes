CREATE TABLE ethereum.blocks 
(
    `block_number` UInt64,           -- Block number
    `block_timestamp` UInt64         -- Block timestamp in seconds
)
ENGINE = MergeTree()
PRIMARY KEY block_number
ORDER BY block_number; 