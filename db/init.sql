CREATE TABLE IF NOT EXISTS projects (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS jobs (
  id BIGSERIAL PRIMARY KEY,
  project BIGINT REFERENCES projects NOT NULL,
  -- Valid states: idle, activating, running, canceling, canceled,
  -- succeeded, failed
  state TEXT NOT NULL DEFAULT 'idle',
  created TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
  started TIMESTAMPTZ,
  finished TIMESTAMPTZ,
  -- An additional layer of priority beyond just getting the
  -- earliest-created available job.
  priority INT NOT NULL DEFAULT 0,
  data JSONB NOT NULL
);
