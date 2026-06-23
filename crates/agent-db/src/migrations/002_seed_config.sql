-- crates/agent-db/src/migrations/002_seed_config.sql
-- Seed default configuration values

INSERT OR IGNORE INTO config (key, value, category, description) VALUES
    ('api_key', '', 'llm', 'DeepSeek API key'),
    ('base_url', 'https://api.deepseek.com/v1', 'llm', 'API base URL'),
    ('default_model', 'deepseek-chat', 'llm', 'Default model name'),
    ('flash_model', 'deepseek-chat', 'llm', 'Model for quick/fast responses'),
    ('pro_model', 'deepseek-reasoner', 'llm', 'Model for complex/reasoning tasks'),
    ('system_prompt', 'You are a helpful AI assistant.', 'llm', 'Default system prompt'),
    ('temperature', '0.7', 'llm', 'Default temperature (0.0-2.0)'),
    ('max_tokens', '4096', 'llm', 'Default max tokens'),
    ('max_tool_iterations', '10', 'llm', 'Max tool-calling loop iterations'),
    ('onboarding_completed', 'false', 'system', 'Whether onboarding wizard is completed'),
    ('onboarding_step', '0', 'system', 'Current onboarding step (0-based)'),
    ('theme', 'dark', 'ui', 'UI theme: dark or light'),
    ('language', 'zh', 'ui', 'Interface language'),
    ('public_url', '', 'system', 'Public-facing URL of this server (for webhooks)');
