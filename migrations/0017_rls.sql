-- Row-Level Security: defense-in-depth tenant isolation at the database layer.
--
-- The app already scopes every tenant query with `WHERE tenant_id = $n` (the
-- belt). RLS is the suspenders: even a future query that forgets the filter
-- cannot cross tenants. It only binds when the connecting role is NOT a
-- superuser/owner (those bypass RLS unconditionally), so the API connects as a
-- dedicated `ferrite_app` role while the worker/migrations keep the owner role.
--
-- Per request the API sets `app.current_tenant` (transaction-local) and every
-- statement is scoped to that tenant. Inherently cross-tenant access (login by
-- email, API-key auth by hash, superadmin console, internal ingest by stream
-- key, background sweeps) sets `app.bypass = on` instead — each such caller is
-- independently authorized in application code.

-- Dedicated non-superuser application role. The password here is a dev default;
-- production overrides it via the API's FERRITE_API_DATABASE_URL credential.
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ferrite_app') THEN
    CREATE ROLE ferrite_app LOGIN PASSWORD 'ferrite_app';
  END IF;
END $$;

GRANT USAGE ON SCHEMA public TO ferrite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO ferrite_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO ferrite_app;
-- Future tables/sequences created by the owner are granted to ferrite_app too,
-- so later migrations don't silently lock the API out.
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ferrite_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO ferrite_app;

-- Enable + FORCE RLS and install the tenant-isolation policy on every
-- tenant-owned table (each has a `tenant_id` column). FORCE keeps the policy in
-- effect even for the table owner, should it ever connect without superuser.
DO $$
DECLARE t text;
BEGIN
  FOREACH t IN ARRAY ARRAY[
    'api_keys','assets','caption_tracks','jobs','live_streams','moderation',
    'playback_events','profiles','provenance','simulcast_targets',
    'transcript_segments','usage','users','webhooks'
  ]
  LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
    EXECUTE format($f$
      CREATE POLICY tenant_isolation ON %I
        USING (
          tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid
          OR current_setting('app.bypass', true) = 'on'
        )
        WITH CHECK (
          tenant_id = NULLIF(current_setting('app.current_tenant', true), '')::uuid
          OR current_setting('app.bypass', true) = 'on'
        )
    $f$, t);
  END LOOP;
END $$;
