SELECT
    source as "source: RunSource",
    commit,
    created_at,
    dequeued_at,
    repo_owner,
    repo_name,
    finished_at as "finished_at?",
    status as "status?: RunStatus",
    execution_time_millis as "execution_time_millis?",
    output as "output?"
FROM runs LEFT OUTER JOIN finished_runs ON runs.id=finished_runs.run WHERE id=$1;
