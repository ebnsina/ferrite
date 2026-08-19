-- sched_db. Separate database from assets_db; neither can read the other.

CREATE TABLE work (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL,
  kind TEXT NOT NULL,
  ref_id UUID NOT NULL,
  spec JSONB NOT NULL,                    -- opaque; the scheduler never reads inside

  lane TEXT NOT NULL,
  priority_key INT NOT NULL DEFAULT 0,    -- lower runs first
  fairness_weight REAL NOT NULL,          -- copied from the plan at submit time

  state TEXT NOT NULL,
  workflow_id TEXT,
  dedupe_key TEXT NOT NULL,
  attempts INT NOT NULL DEFAULT 0,
  last_error TEXT,

  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  admitted_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ,

  CONSTRAINT work_lane_known CHECK (lane IN ('realtime', 'standard', 'bulk')),
  CONSTRAINT work_state_known
    CHECK (state IN ('pending', 'admitted', 'running', 'done', 'failed', 'canceled'))
);

-- Submitting the same key twice returns the first item, never a second one.
CREATE UNIQUE INDEX work_tenant_dedupe ON work (tenant_id, dedupe_key);

-- The admission loop's only hot read.
CREATE INDEX work_pending ON work (lane, tenant_id, priority_key, created_at)
  WHERE state = 'pending';

-- Reconciling in_flight, and finding starts that died mid-flight.
CREATE INDEX work_holding_slot ON work (tenant_id, admitted_at)
  WHERE state IN ('admitted', 'running');

-- Rate limiting reads a one-minute window.
CREATE INDEX work_admitted_recently ON work (tenant_id, admitted_at)
  WHERE admitted_at IS NOT NULL;

CREATE TABLE tenant_budgets (
  tenant_id UUID PRIMARY KEY,
  max_concurrent_tasks INT NOT NULL,
  in_flight INT NOT NULL DEFAULT 0,
  rate_limit_per_min INT NOT NULL,
  fairness_weight REAL NOT NULL DEFAULT 1.0,
  suspended BOOLEAN NOT NULL DEFAULT FALSE,   -- suspension stops admission here
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  CONSTRAINT budgets_in_flight_sane CHECK (in_flight >= 0)
);

-- What a task cost, reported by workers. Rolled up into usage_events by
-- ferrite-assets; the fleet never writes to customer data itself.
CREATE TABLE work_cost (
  work_id UUID PRIMARY KEY REFERENCES work (id) ON DELETE CASCADE,
  tenant_id UUID NOT NULL,
  cpu_seconds DOUBLE PRECISION NOT NULL DEFAULT 0,
  bytes_written BIGINT NOT NULL DEFAULT 0,
  machine TEXT,
  recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX work_cost_tenant ON work_cost (tenant_id, recorded_at);
