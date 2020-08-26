UPDATE jobs
SET state = 'available',
    runner = NULL,
    started = NULL,
    token = NULL
WHERE state = 'running'
  AND (heartbeat +
       make_interval(secs => ((
         SELECT heartbeat_expiration_millis
         FROM projects
         WHERE projects.id = jobs.project) / 1000
       ))) < CURRENT_TIMESTAMP
RETURNING jobs.id
