SELECT
    runs.id,
    commit,
    created_at,
    dequeued_at,
    rain_version,
    repos.id AS repo_id,
    repos.host,
    repos.owner,
    repos.name,
    target,
    finished_at AS "finished_at?",
    status AS "status?",
    execution_time_millis AS "execution_time_millis?",
    output AS "output?"
FROM runs
INNER JOIN repos ON runs.repo=repos.id
LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
ORDER BY runs.id DESC
OFFSET $1 LIMIT $2;
