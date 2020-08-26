UPDATE jobs
SET state = 'running',
    runner = $2,
    started = CURRENT_TIMESTAMP,
    token = $3
WHERE id = (
  SELECT id
  FROM jobs
  WHERE project = (
    SELECT id FROM projects WHERE name = $1
  ) AND state = 'available'
  ORDER BY priority, created
  LIMIT 1
  FOR UPDATE SKIP LOCKED
)
RETURNING id, token
