-- Row-level security on every tenant-scoped table.
--
-- The point is not to stop an attacker with SQL access: it is to make a query
-- that forgets its tenant filter return nothing instead of everything. FORCE
-- applies the policies to the table owner too, so the service connection is
-- subject to them like anyone else.

ALTER TABLE work ENABLE ROW LEVEL SECURITY;
ALTER TABLE work FORCE ROW LEVEL SECURITY;

ALTER TABLE tenant_budgets ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_budgets FORCE ROW LEVEL SECURITY;

ALTER TABLE work_cost ENABLE ROW LEVEL SECURITY;
ALTER TABLE work_cost FORCE ROW LEVEL SECURITY;

-- Admission is cross-tenant by nature: it compares tenants against each other
-- to decide who runs next. That work sets ferrite.scope instead of a tenant.
CREATE POLICY service_scope ON work
  USING (current_setting('ferrite.scope', true) = 'service')
  WITH CHECK (current_setting('ferrite.scope', true) = 'service');

CREATE POLICY service_scope ON tenant_budgets
  USING (current_setting('ferrite.scope', true) = 'service')
  WITH CHECK (current_setting('ferrite.scope', true) = 'service');

CREATE POLICY service_scope ON work_cost
  USING (current_setting('ferrite.scope', true) = 'service')
  WITH CHECK (current_setting('ferrite.scope', true) = 'service');

-- NULLIF, because an unset variable reads as the empty string, which does not
-- cast to uuid. Unset must mean "no rows", not "error".
CREATE POLICY tenant_scope ON work
  USING (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid);

CREATE POLICY tenant_scope ON tenant_budgets
  USING (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid);

CREATE POLICY tenant_scope ON work_cost
  USING (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_id = nullif(current_setting('ferrite.tenant_id', true), '')::uuid);
