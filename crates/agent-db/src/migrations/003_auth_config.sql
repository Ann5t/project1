-- crates/agent-db/src/migrations/003_auth_config.sql
-- Seed authentication configuration values

INSERT OR IGNORE INTO config (key, value, category, description) VALUES
    ('auth_enabled', 'false', 'system', 'Whether token-based authentication is enabled'),
    ('admin_token', '', 'system', 'Admin API token for Bearer authentication');
