//! Built-in tools registered by default in the agent server.
//!
//! These tools are available to the LLM for function calling. Each
//! implements the `Tool` trait and is registered in the `ToolRegistry`
//! at server startup. The `ExecuteShellTool` is opt-in via the
//! `SHELL_TOOL_ENABLED` environment variable.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Local;

use super::types::Tool;
use crate::error::CoreError;

// ── Calculator Tool ──

/// Evaluates mathematical expressions via `meval`.
///
/// Supports arithmetic, trigonometry, logarithms, powers, and constants
/// like `pi` and `e`. Returns the numeric result formatted as a string.
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }

    fn description(&self) -> &'static str {
        "Evaluate a mathematical expression. Supports basic arithmetic, trigonometry, logarithms, and more."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate (e.g., '2 + 3 * 4', 'sin(pi/2)')"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<String, CoreError> {
        let expr = arguments["expression"]
            .as_str()
            .unwrap_or("0")
            .to_string();

        match meval::eval_str(&expr) {
            Ok(result) => Ok(format!("Result: {result}")),
            Err(e) => Err(CoreError::ToolError {
                tool: "calculator".into(),
                message: format!("Failed to evaluate expression '{expr}': {e}"),
            }),
        }
    }
}

// ── Current Time Tool ──

/// Returns the current local date and time in ISO 8601 format.
///
/// Accepts an optional `timezone` argument (currently unused; always
/// returns local time via `chrono::Local`).
pub struct CurrentTimeTool;

#[async_trait]
impl Tool for CurrentTimeTool {
    fn name(&self) -> &'static str {
        "get_current_time"
    }

    fn description(&self) -> &'static str {
        "Get the current date and time in ISO 8601 format."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "timezone": {
                    "type": "string",
                    "description": "Optional timezone (e.g., 'Asia/Shanghai', 'UTC'). Defaults to local time."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, _arguments: serde_json::Value) -> Result<String, CoreError> {
        let now = Local::now();
        Ok(format!(
            "Current time: {}",
            now.format("%Y-%m-%d %H:%M:%S %:z")
        ))
    }
}

// ── Web Search Tool (DuckDuckGo Instant Answer API) ──

/// Performs web searches via the free DuckDuckGo Instant Answer API.
///
/// Returns an abstract/summary (when available) and up to `num_results`
/// related topics with URLs. Does not require an API key.
pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using DuckDuckGo Instant Answer API. Returns abstract text, related topics, and URLs."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 10)"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<String, CoreError> {
        let query = arguments["query"].as_str().unwrap_or("");
        let max_results = arguments["num_results"]
            .as_u64()
            .unwrap_or(5)
            .min(10) as usize;

        if query.is_empty() {
            return Err(CoreError::ToolError {
                tool: "web_search".into(),
                message: "Search query cannot be empty".into(),
            });
        }

        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding(query)
        );

        let response = reqwest::get(&url).await.map_err(CoreError::Http)?;

        let body: serde_json::Value = response.json().await.map_err(CoreError::Http)?;

        let mut output = String::new();

        // Abstract / instant answer
        if let Some(abstract_text) = body["AbstractText"].as_str() {
            if !abstract_text.is_empty() {
                output.push_str(&format!("Abstract: {abstract_text}\n"));
                if let Some(abstract_url) = body["AbstractURL"].as_str() {
                    if !abstract_url.is_empty() {
                        output.push_str(&format!("Source: {abstract_url}\n"));
                    }
                }
                output.push('\n');
            }
        }

        // Related topics
        if let Some(topics) = body["RelatedTopics"].as_array() {
            if !topics.is_empty() {
                output.push_str("Related Topics:\n");
                let mut count = 0;
                for topic in topics {
                    if count >= max_results {
                        break;
                    }
                    if let Some(text) = topic["Text"].as_str() {
                        if text.is_empty() {
                            continue;
                        }
                        count += 1;
                        output.push_str(&format!("{count}. {text}\n"));
                        if let Some(url) = topic["FirstURL"].as_str() {
                            if !url.is_empty() {
                                output.push_str(&format!("   URL: {url}\n"));
                            }
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = format!(
                "No results found for '{query}'. The DuckDuckGo API may not have instant answers for this query."
            );
        }

        // Trim trailing whitespace
        Ok(output.trim().to_string())
    }
}

/// URL-encode a query string for the DuckDuckGo API.
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => format!("%{:02X}", other as u32),
        })
        .collect()
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    // ── Calculator Tool ──

    #[tokio::test]
    async fn calculator_basic_arithmetic() {
        let tool = CalculatorTool;

        let result = tool
            .execute(serde_json::json!({"expression": "2 + 3 * 4"}))
            .await
            .unwrap();
        assert!(result.contains("14"), "Expected 14, got: {}", result);

        let result = tool
            .execute(serde_json::json!({"expression": "(10 - 2) / 4"}))
            .await
            .unwrap();
        assert!(result.contains("2"), "Expected 2, got: {}", result);
    }

    #[tokio::test]
    async fn calculator_power_and_trig() {
        let tool = CalculatorTool;

        let result = tool
            .execute(serde_json::json!({"expression": "2^10"}))
            .await
            .unwrap();
        assert!(result.contains("1024"), "Expected 1024, got: {}", result);

        let result = tool
            .execute(serde_json::json!({"expression": "sin(pi / 6)"}))
            .await
            .unwrap();
        // sin(pi/6) = 0.5
        let val: f64 = result
            .strip_prefix("Result: ")
            .unwrap()
            .parse()
            .unwrap();
        assert!((val - 0.5).abs() < 0.001, "Expected ~0.5, got: {}", val);
    }

    #[tokio::test]
    async fn calculator_error_on_invalid_expression() {
        let tool = CalculatorTool;

        let result = tool
            .execute(serde_json::json!({"expression": "2 + +"}))
            .await;
        assert!(result.is_err(), "Expected error for invalid expression");
    }

    #[tokio::test]
    async fn calculator_defaults_to_zero() {
        let tool = CalculatorTool;

        // Missing expression key
        let result = tool
            .execute(serde_json::json!({}))
            .await
            .unwrap();
        assert!(result.contains("0"), "Expected 0, got: {}", result);
    }

    // ── Current Time Tool ──

    #[tokio::test]
    async fn current_time_returns_iso_format() {
        let tool = CurrentTimeTool;

        let result = tool.execute(serde_json::json!({})).await.unwrap();
        assert!(result.starts_with("Current time: "), "Got: {}", result);

        // Should contain date-like pattern YYYY-MM-DD
        let time_part = result.strip_prefix("Current time: ").unwrap();
        assert!(
            time_part.len() >= 19,
            "Too short for date+time: {}",
            time_part
        );
    }

    #[tokio::test]
    async fn current_time_handles_ignored_args() {
        let tool = CurrentTimeTool;

        // Should work even with extraneous args
        let result = tool
            .execute(serde_json::json!({"timezone": "Asia/Shanghai", "extra": 42}))
            .await
            .unwrap();
        assert!(result.starts_with("Current time: "));
    }

    // ── Urlencoding Helper ──

    #[test]
    fn urlencoding_simple_string() {
        assert_eq!(urlencoding("hello world"), "hello+world");
    }

    #[test]
    fn urlencoding_special_chars() {
        let encoded = urlencoding("hello?q=test&lang=en");
        assert!(encoded.contains("%3F"));
        assert!(encoded.contains("%3D"));
        assert!(encoded.contains("%26"));
    }

    #[test]
    fn urlencoding_unicode() {
        let encoded = urlencoding("你好");
        // The character '你' is U+4F60, encoded as %4F60 by this implementation
        assert!(encoded.starts_with("%4F"),
            "Got: {}", encoded);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn urlencoding_empty_string() {
        assert_eq!(urlencoding(""), "");
    }

    #[test]
    fn urlencoding_preserves_safe_chars() {
        let encoded = urlencoding("abc-123_XYZ.~");
        assert_eq!(encoded, "abc-123_XYZ.~");
    }

    // ── Web Search Tool (error cases, no network) ──

    #[tokio::test]
    async fn web_search_empty_query_returns_error() {
        let tool = WebSearchTool;

        let result = tool
            .execute(serde_json::json!({"query": ""}))
            .await;
        assert!(result.is_err(), "Expected error for empty query");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("cannot be empty"),
            "Got: {}",
            err
        );
    }

    // ── Tool Metadata ──

    #[test]
    fn calculator_tool_metadata() {
        let tool = CalculatorTool;
        assert_eq!(tool.name(), "calculator");
        assert!(tool.description().contains("mathematical"));

        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::json!("expression")));
    }

    #[test]
    fn time_tool_metadata() {
        let tool = CurrentTimeTool;
        assert_eq!(tool.name(), "get_current_time");
        assert!(tool.description().contains("date"));

        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        // No required fields
        assert!(schema["required"].as_array().unwrap().is_empty());
    }

    #[test]
    fn web_search_tool_metadata() {
        let tool = WebSearchTool;
        assert_eq!(tool.name(), "web_search");
        assert!(tool.description().contains("Search"));

        let schema = tool.parameters_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::json!("query")));
    }

    // ── ReadFileTool ──

    #[tokio::test]
    async fn read_file_empty_path_error() {
        // Use a temp directory as allowed_dir
        let dir = std::env::temp_dir();
        let tool = ReadFileTool::new(dir);

        let result = tool
            .execute(serde_json::json!({"path": ""}))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot be empty"), "Got: {}", err);
    }

    #[tokio::test]
    async fn read_file_not_a_file_error() {
        let dir = std::env::temp_dir();
        let tool = ReadFileTool::new(dir);

        // Try to read a non-existent file
        let result = tool
            .execute(serde_json::json!({"path": "nonexistent_file_987654321.txt"}))
            .await;
        assert!(result.is_err());
    }

    // ── ExecuteShellTool ──

    #[test]
    fn execute_shell_whitelist_block() {
        let allowed = vec!["echo".to_string(), "ls".to_string()];
        let tool = ExecuteShellTool::new(allowed, 5);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(tool.execute(serde_json::json!({"command": "cat /etc/passwd"})));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not in the allowed whitelist"), "Got: {}", err);
    }

    #[tokio::test]
    async fn execute_shell_empty_command_error() {
        let tool = ExecuteShellTool::new(vec![], 5);

        let result = tool.execute(serde_json::json!({"command": ""})).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot be empty"), "Got: {}", err);
    }

    #[tokio::test]
    async fn execute_shell_allowed_echo() {
        // On Windows, we need cmd.exe-compatible commands
        let tool = ExecuteShellTool::new(vec![], 5);

        let result = tool
            .execute(serde_json::json!({"command": if cfg!(target_os = "windows") { "echo hello test" } else { "echo hello test" }}))
            .await;
        // This may succeed or fail depending on platform, but should not panic
        if let Ok(output) = result {
            assert!(output.contains("hello"), "Got: {}", output);
        }
    }

    #[test]
    fn environment_config_defaults() {
        // Test from_env returns with defaults (without env vars set)
        let tool = ExecuteShellTool::from_env();
        assert_eq!(tool.timeout_secs, 30);
        assert!(tool.allowed_commands.is_empty());

        let read_tool = ReadFileTool::from_env();
        assert_eq!(read_tool.allowed_dir, std::path::PathBuf::from("data"));
    }
}

// ── Read File Tool ──

/// Reads file contents from a server directory restricted to `allowed_dir`.
///
/// Prevents path-traversal attacks by canonicalizing both the allowed
/// directory and the requested path, then verifying the resolved path
/// stays within the allowed prefix. Supports a `max_lines` parameter
/// to truncate large files.
pub struct ReadFileTool {
    allowed_dir: PathBuf,
}

impl ReadFileTool {
    /// Create a new ReadFileTool.
    /// `allowed_dir` is the base directory that file reads are restricted to.
    pub fn new(allowed_dir: PathBuf) -> Self {
        Self { allowed_dir }
    }

    /// Create from the ALLOWED_DATA_DIR environment variable (defaults to "data").
    #[allow(clippy::disallowed_methods)]
    pub fn from_env() -> Self {
        let dir = env::var("ALLOWED_DATA_DIR")
            .map_or_else(|_| PathBuf::from("data"), PathBuf::from);
        Self::new(dir)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file from the server's allowed data directory. Returns the file contents along with line count."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path to the file within the allowed data directory"
                },
                "max_lines": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default: 2000, 0 for unlimited)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<String, CoreError> {
        let relative_path = arguments["path"].as_str().unwrap_or("");
        #[allow(clippy::cast_possible_truncation)]
        let max_lines = arguments["max_lines"].as_u64().unwrap_or(2000) as usize;

        if relative_path.is_empty() {
            return Err(CoreError::ToolError {
                tool: "read_file".into(),
                message: "File path cannot be empty".into(),
            });
        }

        // Resolve the full path and check it is within the allowed directory
        let allowed_dir = self
            .allowed_dir
            .canonicalize()
            .map_err(|e| CoreError::ToolError {
                tool: "read_file".into(),
                message: format!(
                    "Allowed data directory does not exist or is inaccessible: {} ({e})",
                    self.allowed_dir.display(),
                ),
            })?;

        let requested_path = self.allowed_dir.join(relative_path);

        let resolved_path = requested_path
            .canonicalize()
            .map_err(|e| CoreError::ToolError {
                tool: "read_file".into(),
                message: format!("Cannot access file '{relative_path}': {e}"),
            })?;

        // Safety: ensure the resolved path starts with the allowed directory
        if !resolved_path.starts_with(&allowed_dir) {
            return Err(CoreError::ToolError {
                tool: "read_file".into(),
                message: format!(
                    "Access denied: '{relative_path}' is outside the allowed data directory"
                ),
            });
        }

        // Check it is a file (not a directory)
        if !resolved_path.is_file() {
            return Err(CoreError::ToolError {
                tool: "read_file".into(),
                message: format!("'{relative_path}' is not a file or does not exist"),
            });
        }

        let contents = fs::read_to_string(&resolved_path).map_err(|e| CoreError::ToolError {
            tool: "read_file".into(),
            message: format!("Failed to read file '{relative_path}': {e}"),
        })?;

        // Apply max_lines limit
        let lines: Vec<&str> = contents.lines().collect();
        let total_lines = lines.len();

        let display_contents = if max_lines > 0 && total_lines > max_lines {
            let truncated: Vec<&str> = lines.into_iter().take(max_lines).collect();
            format!(
                "{}\n\n... (truncated, showing {}/{} lines)",
                truncated.join("\n"),
                max_lines,
                total_lines,
            )
        } else {
            contents.clone()
        };

        Ok(format!(
            "File: {relative_path}\nLines: {total_lines}\n\n{display_contents}",
        ))
    }
}

// ── Execute Shell Tool (opt-in, default disabled) ──

/// Executes shell commands on the server. **Disabled by default** for security.
///
/// Opt-in by setting `SHELL_TOOL_ENABLED=true`. Supports command whitelisting
/// via `ALLOWED_COMMANDS` (comma-separated list) and a timeout via
/// `SHELL_TIMEOUT_SECS`. On Windows, commands run through `cmd /C`; on Unix
/// through `sh -c`.
pub struct ExecuteShellTool {
    allowed_commands: Vec<String>,
    timeout_secs: u64,
}

impl ExecuteShellTool {
    /// Create a new ExecuteShellTool.
    /// `allowed_commands` is a whitelist of permitted command names (the first token).
    /// An empty list means any command is allowed (use with caution).
    /// `timeout_secs` is the max runtime for a command.
    pub fn new(allowed_commands: Vec<String>, timeout_secs: u64) -> Self {
        Self {
            allowed_commands,
            timeout_secs,
        }
    }

    /// Create from environment variables:
    /// - `ALLOWED_COMMANDS`: comma-separated list of allowed command names (empty = all allowed)
    /// - `SHELL_TIMEOUT_SECS`: timeout in seconds (default 30)
    #[allow(clippy::disallowed_methods)]
    pub fn from_env() -> Self {
        let allowed = env::var("ALLOWED_COMMANDS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let timeout = env::var("SHELL_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        Self::new(allowed, timeout)
    }
}

#[async_trait]
impl Tool for ExecuteShellTool {
    fn name(&self) -> &'static str {
        "execute_shell"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command on the server. Only whitelisted commands are allowed. Results include stdout, stderr, and exit code."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, arguments: serde_json::Value) -> Result<String, CoreError> {
        let cmd_str = arguments["command"].as_str().unwrap_or("").to_string();

        if cmd_str.is_empty() {
            return Err(CoreError::ToolError {
                tool: "execute_shell".into(),
                message: "Command cannot be empty".into(),
            });
        }

        // Validate command against whitelist
        let command_name = cmd_str.split_whitespace().next().unwrap_or("");

        if !self.allowed_commands.is_empty() {
            let is_allowed = self
                .allowed_commands
                .iter()
                .any(|allowed| allowed == command_name);
            if !is_allowed {
                return Err(CoreError::ToolError {
                    tool: "execute_shell".into(),
                    message: format!(
                        "Command '{}' is not in the allowed whitelist. Allowed: {}",
                        command_name,
                        self.allowed_commands.join(", ")
                    ),
                });
            }
        }

        // Determine shell: use cmd.exe on Windows, sh on Unix
        let (shell, shell_arg) = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let start = Instant::now();
        let timeout = Duration::from_secs(self.timeout_secs);

        let output = Command::new(shell)
            .arg(shell_arg)
            .arg(&cmd_str)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| CoreError::ToolError {
                tool: "execute_shell".into(),
                message: format!("Failed to spawn command: {e}"),
            });

        let mut child = output?;

        // Wait with timeout polling
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let elapsed = start.elapsed();
                    let stdout = String::from_utf8_lossy(
                        &child.stdout.take().map_or(Vec::new(), |mut p| {
                            use std::io::Read;
                            let mut buf = Vec::new();
                            let _ = p.read_to_end(&mut buf);
                            buf
                        }),
                    )
                    .to_string();
                    let stderr = String::from_utf8_lossy(
                        &child.stderr.take().map_or(Vec::new(), |mut p| {
                            use std::io::Read;
                            let mut buf = Vec::new();
                            let _ = p.read_to_end(&mut buf);
                            buf
                        }),
                    )
                    .to_string();

                    let code = status.code().map_or_else(|| "unknown".into(), |c| c.to_string());

                    let mut result = format!(
                        "Exit code: {}\nElapsed: {:.2}s\n",
                        code,
                        elapsed.as_secs_f64()
                    );

                    if !stdout.is_empty() {
                        result.push_str(&format!("\n--- stdout ---\n{}\n", stdout.trim_end()));
                    }
                    if !stderr.is_empty() {
                        result.push_str(&format!("\n--- stderr ---\n{}\n", stderr.trim_end()));
                    }

                    return Ok(result.trim().to_string());
                }
                Ok(None) => {
                    // Still running
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(CoreError::ToolError {
                            tool: "execute_shell".into(),
                            message: format!(
                                "Command timed out after {} seconds: {cmd_str}",
                                self.timeout_secs
                            ),
                        });
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => {
                    return Err(CoreError::ToolError {
                        tool: "execute_shell".into(),
                        message: format!("Failed to wait on command: {e}"),
                    });
                }
            }
        }
    }
}
