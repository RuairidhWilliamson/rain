SELECT
    id,
    host,
    owner,
    name
FROM repos
WHERE id=$1;
