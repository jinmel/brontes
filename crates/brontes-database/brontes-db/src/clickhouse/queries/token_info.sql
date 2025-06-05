SELECT
    address,
    symbol,
    decimals
FROM brontes.token_info
WHERE address = ?
LIMIT 1

