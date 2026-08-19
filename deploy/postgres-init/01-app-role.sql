-- The application must not connect as a superuser: superusers bypass row-level
-- security outright, FORCE included, which makes every policy decorative.
--
-- ferrite_app owns the schema and is subject to its own policies.

CREATE ROLE ferrite_app WITH LOGIN PASSWORD 'ferrite-app-dev' NOSUPERUSER NOBYPASSRLS;

-- The database name is only known at runtime, hence the dynamic statement.
DO $$
BEGIN
  EXECUTE format('GRANT ALL ON DATABASE %I TO ferrite_app', current_database());
END
$$;

ALTER SCHEMA public OWNER TO ferrite_app;
GRANT ALL ON SCHEMA public TO ferrite_app;
