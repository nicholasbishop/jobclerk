CREATE TABLE IF NOT EXISTS projects (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS runners (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE

  -- TODO: will probably add more fields here, not sure yet how
  -- generic to make this. For my use case these runners will probably
  -- be separate EC2 instances.
);

CREATE TABLE IF NOT EXISTS jobs (
  id BIGSERIAL PRIMARY KEY,
  project BIGINT REFERENCES projects NOT NULL,
  runner BIGINT REFERENCES runners,

  -- Valid states: available, running, canceling, canceled, succeeded,
  -- failed
  state TEXT NOT NULL DEFAULT 'available',

  -- Time that the job was created
  created TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

  -- Time that the job was started
  started TIMESTAMPTZ,

  -- Time that the job was either canceled, or it succeeded or failed
  finished TIMESTAMPTZ,

  -- Time that the last heartbeat was received from the job's owner
  heartbeat TIMESTAMPTZ,

  -- When a job is taken (moved from available to running) the token
  -- is set to a random value. The runner that took the job must use
  -- this token to update the job.
  --
  -- This handles the following case:
  -- 1. Client Alpha starts running the job
  -- 2. Alpha gets stuck and stops sending a heartbeat
  -- 3. The job gets moved back to the available state
  -- 4. Client Beta starts running the job
  -- 5. Alpha gets unstuck and continues running the job
  -- 6. Without the token, this would result in conflicting updates
  --    from Alpha and Beta. With the token, the updates from Alpha
  --    can be rejected (and assuming Alpha is paying attention to the
  --    response, it can stop trying to run the job).
  token TEXT,

  -- An additional layer of priority beyond just getting the
  -- earliest-created available job.
  priority INT NOT NULL DEFAULT 0,

  -- Arbitrary JSON payload
  data JSONB NOT NULL
);
