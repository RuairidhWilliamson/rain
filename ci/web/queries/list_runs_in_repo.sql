SELECT
    runs.id,
    commit,
    created_at,
    dequeued_at,
    repos.id as repo_id,
    repos.host,
    repos.owner,
    repos.name,
    target,
    finished_at as "finished_at?",
    status as "status?",
    execution_time_millis as "execution_time_millis?",
    output as "output?"
FROM runs
INNER JOIN repos ON runs.repo=repos.id
LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run
WHERE repos.id=$1
ORDER BY runs.id DESC LIMIT 100;
