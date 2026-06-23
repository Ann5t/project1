use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

use futures::stream::StreamExt;

use super::types::{StepResult, StepStatus, StepType, WorkflowDefinition, WorkflowResult, WorkflowStep};
use crate::error::CoreError;
use crate::llm::client::LlmClient;
use crate::tool::registry::ToolRegistry;

/// Workflow execution engine that runs a DAG of steps.
///
/// Execution proceeds via topological sort (Kahn's algorithm):
///
/// 1. Compute in-degrees and adjacency lists from the workflow's edges.
/// 2. Push all steps with in-degree 0 onto a queue.
/// 3. Execute all queued steps in parallel as a batch.
/// 4. For each completed step, decrement the in-degree of downstream
///    neighbours. When a neighbour's in-degree reaches 0, enqueue it.
/// 5. If a step fails, all downstream steps are marked `Skipped`.
/// 6. Continue until the queue is empty.
///
/// See [`WorkflowEngine::execute`] for the entry point.
pub struct WorkflowEngine {
    llm: Arc<dyn LlmClient>,
    #[expect(dead_code, reason = "Reserved for future ToolCall step implementation (Phase 9)")]
    tools: Arc<ToolRegistry>,
}

impl WorkflowEngine {
    /// Create a new workflow engine with the given LLM client and tool registry.
    pub fn new(llm: Arc<dyn LlmClient>, tools: Arc<ToolRegistry>) -> Self {
        Self { llm, tools }
    }

    /// Execute a workflow definition, returning results
    pub async fn execute(
        &self,
        workflow_id: &str,
        definition: &WorkflowDefinition,
    ) -> Result<WorkflowResult, CoreError> {
        let run_id = Uuid::new_v4().to_string();
        info!(
            "Starting workflow '{}' run {} ({} steps)",
            definition.name,
            run_id,
            definition.steps.len()
        );

        // ── Step 0: Validate edges reference existing steps ──
        let step_ids: std::collections::HashSet<&str> =
            definition.steps.iter().map(|s| s.id.as_str()).collect();

        for edge in &definition.edges {
            if !step_ids.contains(edge.source.as_str()) {
                return Err(CoreError::InvalidConfig(format!(
                    "Edge '{}' references source step '{}' which is not defined in steps",
                    edge.id, edge.source
                )));
            }
            if !step_ids.contains(edge.target.as_str()) {
                return Err(CoreError::InvalidConfig(format!(
                    "Edge '{}' references target step '{}' which is not defined in steps",
                    edge.id, edge.target
                )));
            }
        }

        // Build adjacency and in-degree maps for DAG execution
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        for step in &definition.steps {
            adjacency.entry(step.id.clone()).or_default();
            in_degree.entry(step.id.clone()).or_insert(0);
        }

        for edge in &definition.edges {
            adjacency
                .entry(edge.source.clone())
                .or_default()
                .push(edge.target.clone());
            *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
        }

        // ── Step 0b: Detect circular dependencies (Kahn's algorithm dry-run) ──
        {
            let mut temp_in_degree = in_degree.clone();
            let mut visited: VecDeque<String> = VecDeque::new();
            for step in &definition.steps {
                if *temp_in_degree.get(&step.id).unwrap_or(&0) == 0 {
                    visited.push_back(step.id.clone());
                }
            }
            let mut processed = 0usize;
            while let Some(node) = visited.pop_front() {
                processed += 1;
                if let Some(neighbors) = adjacency.get(&node) {
                    for neighbor in neighbors {
                        if let Some(deg) = temp_in_degree.get_mut(neighbor) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                visited.push_back(neighbor.clone());
                            }
                        }
                    }
                }
            }
            if processed != definition.steps.len() {
                let remaining: Vec<&str> = temp_in_degree
                    .iter()
                    .filter(|(_, &d)| d > 0)
                    .map(|(id, _)| id.as_str())
                    .collect();
                return Err(CoreError::InvalidConfig(format!(
                    "Circular dependency detected in workflow: {} step(s) in cycle: {:?}",
                    remaining.len(),
                    remaining
                )));
            }
        }

        // Topological sort + parallel execution
        let mut queue: VecDeque<String> = VecDeque::new();
        for step in &definition.steps {
            if *in_degree.get(&step.id).unwrap_or(&0) == 0 {
                queue.push_back(step.id.clone());
            }
        }

        let mut results: HashMap<String, StepResult> = HashMap::new();

        while !queue.is_empty() {
            // Execute all currently available steps (same level = parallel)
            let current_batch: Vec<String> = queue.drain(..).collect();
            let batch_results = self.execute_batch(
                definition,
                &current_batch,
                &results,
            ).await;

            // Process results
            for result in batch_results {
                let step_id = result.step_id.clone();
                let success = result.status == StepStatus::Success;
                results.insert(step_id.clone(), result);

                // If step failed, skip downstream steps
                if !success {
                    warn!("Step {} failed in workflow {}", step_id, workflow_id);
                    // Mark downstream as skipped
                    if let Some(downstream) = adjacency.get(&step_id) {
                        for target in downstream {
                            results.insert(target.clone(), StepResult {
                                step_id: target.clone(),
                                status: StepStatus::Skipped,
                                output: None,
                                error: Some("Upstream step failed".into()),
                            });
                        }
                    }
                    continue;
                }

                // Decrement in-degree of downstream steps
                if let Some(downstream) = adjacency.get(&step_id) {
                    for target in downstream {
                        if let Some(deg) = in_degree.get_mut(target) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                queue.push_back(target.clone());
                            }
                        }
                    }
                }
            }
        }

        let overall_status = if results.values().any(|r| r.status == StepStatus::Error) {
            StepStatus::Error
        } else if results.values().all(|r| r.status == StepStatus::Skipped) {
            StepStatus::Skipped
        } else {
            StepStatus::Success
        };

        let steps: Vec<StepResult> = results.into_values().collect();

        Ok(WorkflowResult {
            workflow_id: workflow_id.to_string(),
            run_id,
            status: overall_status,
            steps,
            publish_url: None,
        })
    }

    /// Execute a batch of independent steps in parallel using `StreamExt::buffer_unordered`.
    ///
    /// Steps at the same topological level are independent by definition, so running them
    /// concurrently (up to a concurrency cap of 12) is safe and substantially faster than
    /// sequential iteration.
    async fn execute_batch(
        &self,
        definition: &WorkflowDefinition,
        step_ids: &[String],
        _previous_results: &HashMap<String, StepResult>,
    ) -> Vec<StepResult> {
        let llm = Arc::clone(&self.llm);

        let step_map: HashMap<&str, &WorkflowStep> =
            definition.steps.iter().map(|s| (s.id.as_str(), s)).collect();

        // Build a stream of futures, one per step.
        let futures = step_ids.iter().map(|step_id| {
            let step_id = step_id.clone();
            let step_opt = step_map.get(step_id.as_str()).copied().cloned();
            let llm = Arc::clone(&llm);

            async move {
                match step_opt {
                    Some(step) => execute_single_step(llm, &step_id, &step).await,
                    None => StepResult {
                        step_id,
                        status: StepStatus::Error,
                        output: None,
                        error: Some("Step not found".into()),
                    },
                }
            }
        });

        // buffer_unordered(N) limits concurrent step execution to N at a time.
        futures::stream::iter(futures)
            .buffer_unordered(12)
            .inspect(|r| debug!("Step {} result: {:?}", r.step_id, r.status))
            .collect()
            .await
    }
}

/// Standalone async function to execute a single workflow step.
///
/// Takes ownership of the data it needs (via `Arc`) so it can be spawned
/// alongside other steps in a `buffer_unordered` or `JoinSet`.
async fn execute_single_step(
    llm: Arc<dyn LlmClient>,
    step_id: &str,
    step: &WorkflowStep,
) -> StepResult {
    match step.step_type {
        StepType::Delay => {
            let seconds = step.config["seconds"].as_u64().unwrap_or(5);
            tokio::time::sleep(Duration::from_secs(seconds)).await;
            StepResult {
                step_id: step_id.to_string(),
                status: StepStatus::Success,
                output: Some(format!("Waited {seconds} seconds")),
                error: None,
            }
        }
        StepType::LlmCall => {
            let prompt = step.config["prompt"].as_str().unwrap_or("Hello");
            let model = step.config["model"].as_str().unwrap_or("deepseek-chat");

            let request = crate::llm::types::ChatRequest {
                model: model.to_string(),
                messages: vec![crate::llm::types::ChatMessage {
                    role: "user".into(),
                    content: prompt.to_string(),
                    tool_calls: None,
                    tool_call_id: None,
                }],
                tools: None,
                temperature: Some(0.7),
                max_tokens: Some(4096),
                stream: false,
            };

            // Wrap LLM call with a 3-minute timeout so a hung API does not stall the batch.
            let chat_future = llm.chat(&request);
            match tokio::time::timeout(Duration::from_secs(180), chat_future).await {
                Ok(Ok(resp)) => {
                    let content = resp
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();
                    StepResult {
                        step_id: step_id.to_string(),
                        status: StepStatus::Success,
                        output: Some(content),
                        error: None,
                    }
                }
                Ok(Err(e)) => StepResult {
                    step_id: step_id.to_string(),
                    status: StepStatus::Error,
                    output: None,
                    error: Some(e.to_string()),
                },
                Err(_elapsed) => StepResult {
                    step_id: step_id.to_string(),
                    status: StepStatus::Error,
                    output: None,
                    error: Some("LLM call timed out after 180s".into()),
                },
            }
        }
        StepType::Publish => StepResult {
            step_id: step_id.to_string(),
            status: StepStatus::Success,
            output: Some("Publish step placeholder".into()),
            error: None,
        },
        StepType::Condition => StepResult {
            step_id: step_id.to_string(),
            status: StepStatus::Success,
            output: Some("Condition step placeholder".into()),
            error: None,
        },
        StepType::ToolCall => StepResult {
            step_id: step_id.to_string(),
            status: StepStatus::Success,
            output: Some("Tool call step placeholder".into()),
            error: None,
        },
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use super::types::WorkflowEdge;
    use crate::llm::types::*;
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Mock LLM client that returns prescribed responses.
    struct MockLlmClient {
        responses: std::collections::HashMap<String, (bool, String)>,
        // (is_success, content_or_error)
    }

    impl MockLlmClient {
        fn new() -> Self {
            Self {
                responses: std::collections::HashMap::new(),
            }
        }

        fn with_response(mut self, prompt: &str, response: &str) -> Self {
            self.responses
                .insert(prompt.to_string(), (true, response.to_string()));
            self
        }

        fn with_error(mut self, prompt: &str, error_msg: &str) -> Self {
            self.responses
                .insert(prompt.to_string(), (false, error_msg.to_string()));
            self
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, CoreError> {
            let prompt = &request.messages[0].content;
            match self.responses.get(prompt) {
                Some((true, content)) => Ok(ChatResponse {
                    id: "mock-1".into(),
                    object: "chat.completion".into(),
                    created: 1,
                    model: request.model.clone(),
                    choices: vec![ChatChoice {
                        index: 0,
                        message: ChatMessage {
                            role: "assistant".into(),
                            content: content.clone(),
                            tool_calls: None,
                            tool_call_id: None,
                        },
                        finish_reason: Some("stop".into()),
                    }],
                }),
                Some((false, err_msg)) => Err(CoreError::LlmApi(err_msg.clone())),
                None => Ok(ChatResponse {
                    id: "mock-default".into(),
                    object: "chat.completion".into(),
                    created: 1,
                    model: request.model.clone(),
                    choices: vec![ChatChoice {
                        index: 0,
                        message: ChatMessage {
                            role: "assistant".into(),
                            content: "Default mock response".into(),
                            tool_calls: None,
                            tool_call_id: None,
                        },
                        finish_reason: Some("stop".into()),
                    }],
                }),
            }
        }

        async fn list_models(&self) -> Result<Vec<String>, CoreError> {
            Ok(vec!["mock-model".into()])
        }

        fn chat_stream(
            &self,
            _request: &ChatRequest,
        ) -> std::pin::Pin<Box<dyn futures::stream::Stream<Item = Result<String, CoreError>> + Send>>
        {
            Box::pin(futures::stream::empty())
        }
    }

    fn make_definition(name: &str, steps: Vec<WorkflowStep>, edges: Vec<WorkflowEdge>) -> WorkflowDefinition {
        WorkflowDefinition {
            name: name.into(),
            description: String::new(),
            steps,
            edges,
        }
    }

    fn make_step(id: &str, name: &str, step_type: StepType, config: serde_json::Value) -> WorkflowStep {
        WorkflowStep {
            id: id.into(),
            name: name.into(),
            step_type,
            config,
            position: None,
        }
    }

    fn make_edge(id: &str, source: &str, target: &str) -> WorkflowEdge {
        WorkflowEdge {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            label: None,
            condition: None,
        }
    }

    // ── Delay step tests ──

    #[tokio::test]
    async fn single_delay_step() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "delay-test",
            vec![make_step(
                "s1",
                "Wait 1s",
                StepType::Delay,
                serde_json::json!({"seconds": 1}),
            )],
            vec![],
        );

        let result = engine.execute("wf-1", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].status, StepStatus::Success);
        assert!(result.steps[0].output.as_ref().unwrap().contains("1 seconds"));
    }

    #[tokio::test]
    async fn delay_step_default_seconds() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "delay-default",
            vec![make_step("s1", "Wait", StepType::Delay, serde_json::json!({}))],
            vec![],
        );

        let result = engine.execute("wf-2", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert!(result.steps[0].output.as_ref().unwrap().contains("5 seconds"));
    }

    // ── LLM step tests ──

    #[tokio::test]
    async fn single_llm_step_success() {
        let llm = Arc::new(
            MockLlmClient::new().with_response("Summarize this", "Summary: Done.")
        );
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "llm-test",
            vec![make_step(
                "s1",
                "Summarize",
                StepType::LlmCall,
                serde_json::json!({"prompt": "Summarize this", "model": "test-model"}),
            )],
            vec![],
        );

        let result = engine.execute("wf-3", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].status, StepStatus::Success);
        assert_eq!(result.steps[0].output.as_deref(), Some("Summary: Done."));
    }

    #[tokio::test]
    async fn llm_step_error_propagates() {
        let llm = Arc::new(
            MockLlmClient::new().with_error("Bad prompt", "Mock API error")
        );
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "llm-error-test",
            vec![make_step(
                "s1",
                "Fail",
                StepType::LlmCall,
                serde_json::json!({"prompt": "Bad prompt"}),
            )],
            vec![],
        );

        let result = engine.execute("wf-4", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Error);
        assert_eq!(result.steps[0].status, StepStatus::Error);
        assert!(result.steps[0].error.as_ref().unwrap().contains("Mock API error"));
    }

    // ── DAG Execution Tests ──

    #[tokio::test]
    async fn simple_linear_dag() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "linear-dag",
            vec![
                make_step("s1", "First", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("s2", "Second", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![make_edge("e1", "s1", "s2")],
        );

        let result = engine.execute("wf-5", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 2);
        for step in &result.steps {
            assert_eq!(step.status, StepStatus::Success, "Step {} failed", step.step_id);
        }
    }

    #[tokio::test]
    async fn two_parallel_branches() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "parallel-dag",
            vec![
                make_step("s1", "Start", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("s2", "Branch A", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("s3", "Branch B", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "s1", "s2"),
                make_edge("e2", "s1", "s3"),
            ],
        );

        let result = engine.execute("wf-6", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 3);
        for step in &result.steps {
            assert_eq!(step.status, StepStatus::Success);
        }
    }

    #[tokio::test]
    async fn diamond_dag_converging() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // start -> A, start -> B, A -> end, B -> end
        let def = make_definition(
            "diamond-dag",
            vec![
                make_step("start", "Start", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("left", "Left", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("right", "Right", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("end", "End", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "start", "left"),
                make_edge("e2", "start", "right"),
                make_edge("e3", "left", "end"),
                make_edge("e4", "right", "end"),
            ],
        );

        let result = engine.execute("wf-7", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 4);
        for step in &result.steps {
            assert_eq!(
                step.status,
                StepStatus::Success,
                "Step {} should succeed, got: {:?}",
                step.step_id,
                step.error
            );
        }
    }

    #[tokio::test]
    async fn failure_skips_downstream() {
        let llm = Arc::new(
            MockLlmClient::new().with_error("fail me", "Fail!")
        );
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "fail-dag",
            vec![
                make_step("s1", "WillFail", StepType::LlmCall, serde_json::json!({"prompt": "fail me"})),
                make_step("s2", "Skipped", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![make_edge("e1", "s1", "s2")],
        );

        let result = engine.execute("wf-8", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Error);

        let s1 = result.steps.iter().find(|s| s.step_id == "s1").unwrap();
        assert_eq!(s1.status, StepStatus::Error);

        let s2 = result.steps.iter().find(|s| s.step_id == "s2").unwrap();
        assert_eq!(s2.status, StepStatus::Skipped);
        assert!(s2.error.as_ref().unwrap().contains("Upstream step failed"));
    }

    // ── Placeholder Step Types ──

    #[tokio::test]
    async fn publish_step_placeholder() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "publish-test",
            vec![make_step("s1", "Publish", StepType::Publish, serde_json::json!({}))],
            vec![],
        );

        let result = engine.execute("wf-9", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert!(result.steps[0].output.as_ref().unwrap().contains("placeholder"));
    }

    #[tokio::test]
    async fn condition_step_placeholder() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "condition-test",
            vec![make_step("s1", "If", StepType::Condition, serde_json::json!({}))],
            vec![],
        );

        let result = engine.execute("wf-10", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert!(result.steps[0].output.as_ref().unwrap().contains("placeholder"));
    }

    #[tokio::test]
    async fn tool_call_step_placeholder() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition(
            "tool-call-test",
            vec![make_step("s1", "Tool", StepType::ToolCall, serde_json::json!({}))],
            vec![],
        );

        let result = engine.execute("wf-11", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert!(result.steps[0].output.as_ref().unwrap().contains("placeholder"));
    }

    // ── Edge Cases ──

    #[tokio::test]
    async fn empty_workflow() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        let def = make_definition("empty", vec![], vec![]);

        let result = engine.execute("wf-empty", &def).await.unwrap();
        // Empty workflow has all steps skipped (vacuous truth on empty iterator)
        assert_eq!(result.status, StepStatus::Skipped);
        assert!(result.steps.is_empty());
    }

    #[tokio::test]
    async fn disconnected_graph_independent_steps() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // Two independent steps with no edges — should both execute
        let def = make_definition(
            "independent",
            vec![
                make_step("a", "A", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("b", "B", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![],
        );

        let result = engine.execute("wf-independent", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 2);
        for step in &result.steps {
            assert_eq!(step.status, StepStatus::Success);
        }
    }

    // ── Edge Cases Added: Circular Dependency ──

    #[tokio::test]
    async fn circular_dependency_detection_rejects() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // A -> B -> C -> A  (cycle)
        let def = make_definition(
            "circular-dag",
            vec![
                make_step("a", "A", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("b", "B", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("c", "C", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "a", "b"),
                make_edge("e2", "b", "c"),
                make_edge("e3", "c", "a"),
            ],
        );

        let result = engine.execute("wf-circ", &def).await;
        assert!(result.is_err(), "Circular dependency should be rejected");
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("circular") || err_msg.contains("cycle"),
            "Error should mention circular/cycle dependency: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn self_loop_dependency_rejects() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // A -> A (self loop)
        let def = make_definition(
            "self-loop",
            vec![
                make_step("s1", "Step", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "s1", "s1"),
            ],
        );

        let result = engine.execute("wf-self-loop", &def).await;
        assert!(result.is_err(), "Self-loop should be rejected as circular");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("circular") || err_msg.contains("cycle") || err_msg.contains("self"),
            "Error should mention cycle: {}",
            err_msg
        );
    }

    // ── Edge Cases Added: Missing Referenced Step ──

    #[tokio::test]
    async fn missing_referenced_step_in_edge_errors() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // Edge references a step that doesn't exist in steps list
        let def = make_definition(
            "missing-step",
            vec![
                make_step("s1", "Real Step", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "s1", "nonexistent_target"),
            ],
        );

        let result = engine.execute("wf-missing", &def).await;
        assert!(result.is_err(), "Missing referenced step should error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not defined in steps"),
            "Error should mention step not defined: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn missing_source_step_in_edge_errors() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // Edge source references a step that doesn't exist
        let def = make_definition(
            "missing-source",
            vec![
                make_step("s1", "Real Step", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "nonexistent_source", "s1"),
            ],
        );

        let result = engine.execute("wf-missing-src", &def).await;
        assert!(result.is_err(), "Missing source step in edge should error");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not defined in steps"),
            "Error should mention step not defined: {}",
            err_msg
        );
    }

    // ── Edge Cases Added: Extended DAG Patterns ──

    #[tokio::test]
    async fn workflow_three_stage_pipeline() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // Stage 1 -> Stage 2a, Stage 2b -> Stage 3
        let def = make_definition(
            "three-stage",
            vec![
                make_step("fetch", "Fetch Data", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("process_a", "Process A", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("process_b", "Process B", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("merge", "Merge Results", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "fetch", "process_a"),
                make_edge("e2", "fetch", "process_b"),
                make_edge("e3", "process_a", "merge"),
                make_edge("e4", "process_b", "merge"),
            ],
        );

        let result = engine.execute("wf-three-stage", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 4);
        for step in &result.steps {
            assert_eq!(
                step.status,
                StepStatus::Success,
                "Step {} should succeed, got: {:?}",
                step.step_id,
                step.error
            );
        }
    }

    #[tokio::test]
    async fn workflow_multiple_independent_chains() {
        let llm = Arc::new(MockLlmClient::new());
        let tools = Arc::new(ToolRegistry::new());
        let engine = WorkflowEngine::new(llm, tools);

        // Two completely independent linear chains
        let def = make_definition(
            "parallel-chains",
            vec![
                make_step("chain1_a", "Chain1 Start", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("chain1_b", "Chain1 End", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("chain2_a", "Chain2 Start", StepType::Delay, serde_json::json!({"seconds": 0})),
                make_step("chain2_b", "Chain2 End", StepType::Delay, serde_json::json!({"seconds": 0})),
            ],
            vec![
                make_edge("e1", "chain1_a", "chain1_b"),
                make_edge("e2", "chain2_a", "chain2_b"),
            ],
        );

        let result = engine.execute("wf-parallel-chains", &def).await.unwrap();
        assert_eq!(result.status, StepStatus::Success);
        assert_eq!(result.steps.len(), 4);
        for step in &result.steps {
            assert_eq!(step.status, StepStatus::Success);
        }
    }
}
