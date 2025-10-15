WITH
    388730456 AS start_block,
    388730459 AS end_block,
    raw_blocks AS (
        SELECT
            block_number,
            block_hash,
            anyLast(block_timestamp) AS block_timestamp
        FROM ethereum.blocks
        WHERE block_number >= start_block AND block_number < end_block AND valid = 1
        GROUP BY block_number, block_hash
    )
SELECT
    CAST(b.block_number, 'UInt64') AS block_number,
    CAST(b.block_hash, 'String') AS block_hash,
    CAST(b.block_timestamp, 'UInt64') AS block_timestamp,
    null as relay_timestamp,
    null as p2p_timestamp,
    null as proposer_fee_recipient,
    null as proposer_mev_reward,
    [] as private_txs
FROM raw_blocks b