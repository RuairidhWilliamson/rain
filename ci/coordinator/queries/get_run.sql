SELECT
    repos.id as "repo_id",
    repos.host,
    repos.owner,
    repos.name,
    commit,
    created_at,
    dequeued_at,
    rain_version,
    target,
    finished_at as "finished_at?",
    status as "status?",
    execution_time_millis as "execution_time_millis?",
    output as "output?"
FROM runs
INNER JOIN repos ON runs.repo=repos.id
LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
WHERE runs.id=$1;
