SELECT
    id,
    host,
    owner,
    name
FROM repos
OFFSET $1 LIMIT $2;
