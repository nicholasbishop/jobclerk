CREATE TABLE IF NOT EXISTS projects (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
  id BIGSERIAL PRIMARY KEY,
  project BIGINT REFERENCES projects NOT NULL,
  -- Valid states: idle, running, canceling, canceled, succeeded,
  -- failed
  state TEXT NOT NULL,
  created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  started TIMESTAMP,
  finished TIMESTAMP,
  -- An additional layer of priority beyond just getting the
  -- earliest-created available job.
  priority INT NOT NULL,
  data JSONB NOT NULL
);
