# AICmd Workflow Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a strict multi-step `workflow` plan that automatically runs read-only checks, confirms all changes once, executes steps in order, verifies the result, and offers at most two confirmed repair plans.

**Architecture:** Extend `ExecutionPlan` with a validated workflow payload and add a focused `workflow_cmd` coordinator. The coordinator prepares decisions and records results; existing `execute_cmd`, `confirm_cmd`, `result_cmd`, and model-call helpers continue to own process execution, confirmation, session formatting, and plan generation.

**Tech Stack:** Rust 2021, existing `serde`/`serde_json`, Tokio, existing AICmd planner/client/session/terminal helpers. No new dependency.

## Global Constraints

- Preserve `direct`, `script`, `search`, and `diagnose` behavior and explicit `do`, `search`, and `err` entry points.
- Only `check` steps declared and locally classified as `read_only` may run before confirmation.
- Any `changes_files`, `changes_system`, or `destructive` step requires one full-plan confirmation; destructive steps retain the existing second confirmation.
- Local risk analysis may raise model-declared risk but never lower it.
- Modification steps never retry automatically.
- A workflow is `completed` only when every required verification step succeeds.
- Any revised plan is strictly validated, displayed, and confirmed again; allow at most two repair cycles.
- Ctrl-C stops the current process and all later steps, preserves captured output, and records `cancelled`.
- Keep AI summary optional and outside completion decisions.
- Do not add parallel steps, background jobs, a generic agent loop, or new configuration keys.
- Keep the supported runtime scope to macOS and Linux; do not add native Windows workflow behavior.
- Documentation remains bilingual. Preserve unrelated local files such as `mcp.json`, `.aicmd/`, `.codebase-memory/`, `.DS_Store`, and `tavily_key.txt`.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/plan_cmd.rs` | Workflow JSON types, strict semantic validation, rendering, and route selection. |
| `assets/roles/%shell%.md` | Planner contract and examples for the new workflow mode. |
| `src/workflow_cmd.rs` | Pre-check evaluation, `run_if` decisions, ordered step state, outcomes, and repair limit. |
| `src/confirm_cmd.rs` | Risk escalation and complete workflow-plan confirmation. |
| `src/execute_cmd.rs` | Existing single-command execution; no workflow orchestration. |
| `src/result_cmd.rs` | Workflow step records and aggregate session note. |
| `src/repair_cmd.rs` | Bilingual workflow repair prompt. |
| `src/main.rs` | Route `workflow` plans and connect model, confirmation, execution, repair, and session boundaries. |
| `src/help_cmd.rs`, `README.md`, `README.en.md`, `docs/aicmd-usage.md` | User-facing workflow behavior and examples. |

---

### Task 1: Define and validate workflow plans

**Files:**
- Modify: `src/plan_cmd.rs`
- Modify: `assets/roles/%shell%.md`

**Interfaces:**
- Produces: `PlanMode::Workflow`, `WorkflowPlan`, `WorkflowStep`, `WorkflowStepKind`, `WorkflowRisk`, `WorkflowFailurePolicy`, `WorkflowCondition`.
- Produces: `ExecutionPlan::workflow() -> Option<WorkflowPlan>` and `RouteKind::Workflow`.
- Consumes: existing `parse_execution_plan`, `render_execution_plan`, and planner model call.

- [ ] **Step 1: Write strict parser tests**

Add these tests to `plan_cmd::tests` before defining the new types:

```rust
#[test]
fn parses_strict_workflow_plan() -> anyhow::Result<()> {
    let plan = parse_execution_plan(r#"{
      "mode":"workflow","command":"","query":"","problem":"","preflight":[],
      "summary":"Install tool",
      "steps":[
        {"id":"check","kind":"check","command":"command -v tool","risk":"read_only","on_failure":"continue"},
        {"id":"install","kind":"action","command":"brew install tool","risk":"changes_system","run_if":{"step":"check","result":"failed"},"on_failure":"stop"},
        {"id":"verify","kind":"verify","command":"tool --version","risk":"read_only","on_failure":"repair"}
      ]
    }"#)?;
    assert_eq!(plan.mode, PlanMode::Workflow);
    assert_eq!(plan.workflow().unwrap().steps.len(), 3);
    Ok(())
}

#[test]
fn rejects_invalid_workflow_relationships() {
    let duplicate = workflow_json_with_steps(r#"[
      {"id":"x","kind":"check","command":"true","risk":"read_only","on_failure":"continue"},
      {"id":"x","kind":"verify","command":"true","risk":"read_only","on_failure":"stop"}
    ]"#);
    assert!(parse_execution_plan(&duplicate).is_err());

    let forward_reference = workflow_json_with_steps(r#"[
      {"id":"install","kind":"action","command":"true","risk":"changes_files","run_if":{"step":"later","result":"failed"},"on_failure":"stop"},
      {"id":"later","kind":"check","command":"false","risk":"read_only","on_failure":"continue"},
      {"id":"verify","kind":"verify","command":"true","risk":"read_only","on_failure":"stop"}
    ]"#);
    assert!(parse_execution_plan(&forward_reference).is_err());
}

#[test]
fn workflow_requires_read_only_verification() {
    let no_verify = workflow_json_with_steps(r#"[
      {"id":"action","kind":"action","command":"touch x","risk":"changes_files","on_failure":"stop"}
    ]"#);
    assert!(parse_execution_plan(&no_verify).is_err());

    let unsafe_check = workflow_json_with_steps(r#"[
      {"id":"check","kind":"check","command":"touch x","risk":"changes_files","on_failure":"stop"},
      {"id":"verify","kind":"verify","command":"test -f x","risk":"read_only","on_failure":"stop"}
    ]"#);
    assert!(parse_execution_plan(&unsafe_check).is_err());
}
```

The test helper must return the complete JSON string, not use permissive partial deserialization:

```rust
fn workflow_json_with_steps(steps: &str) -> String {
    format!(
        r#"{{"mode":"workflow","command":"","query":"","problem":"","preflight":[],"summary":"test","steps":{steps}}}"#
    )
}
```

- [ ] **Step 2: Run the tests and confirm RED**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests -- --nocapture
```

Expected: compilation fails because `PlanMode::Workflow` and `workflow()` do not exist.

- [ ] **Step 3: Add the strict workflow types**

Add these types to `src/plan_cmd.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind { Check, Action, Verify }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRisk { ReadOnly, ChangesFiles, ChangesSystem, Destructive }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowFailurePolicy { Continue, Stop, Repair }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowConditionResult { Passed, Failed }

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowCondition {
    pub step: String,
    pub result: WorkflowConditionResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStep {
    pub id: String,
    pub kind: WorkflowStepKind,
    pub command: String,
    pub risk: WorkflowRisk,
    #[serde(default)]
    pub run_if: Option<WorkflowCondition>,
    pub on_failure: WorkflowFailurePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowPlan {
    pub summary: String,
    pub steps: Vec<WorkflowStep>,
}
```

Add `Workflow` to `PlanMode` and `RouteKind`. Extend `ExecutionPlan` with defaulted fields:

```rust
#[serde(default)]
pub summary: String,
#[serde(default)]
pub steps: Vec<WorkflowStep>,
```

Implement:

```rust
impl ExecutionPlan {
    pub fn workflow(&self) -> Option<WorkflowPlan> {
        (self.mode == PlanMode::Workflow).then(|| WorkflowPlan {
            summary: self.summary.clone(),
            steps: self.steps.clone(),
        })
    }
}
```

Use a `HashSet<String>` in `validate_workflow` to reject empty summary/steps/IDs/commands, duplicate IDs, conditions referencing anything except an earlier `check`, non-read-only `check` or `verify`, and workflows with no `verify`. For non-workflow modes, require `summary.is_empty()` and `steps.is_empty()`.

- [ ] **Step 4: Update the planner role contract**

In `assets/roles/%shell%.md`, add `workflow` as the fifth mode. State in both languages:

```text
Use workflow when the task needs environment checks, one or more changes, and final verification.
需要环境检查、一个或多个修改步骤以及最终验证时使用 workflow。

check and verify steps must be read_only. run_if may reference only an earlier check and supports passed or failed. Include at least one verify step.
check 和 verify 必须是 read_only。run_if 只能引用之前的 check，结果只能是 passed 或 failed。至少包含一个 verify 步骤。
```

Include the exact three-step JSON example from the approved spec. Update the fixed-field contract so `summary` and `steps` are present for every mode and empty outside workflow.

- [ ] **Step 5: Run focused tests and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests
```

Expected: all plan tests pass, including fenced/unknown-field rejection and the new workflow cases.

Commit:

```bash
git add src/plan_cmd.rs 'assets/roles/%shell%.md'
git commit -m "feat: add strict workflow plans"
```

---

### Task 2: Prepare workflow steps from read-only checks

**Files:**
- Create: `src/workflow_cmd.rs`
- Modify: `src/main.rs` (module declaration only in this task)

**Interfaces:**
- Consumes: `WorkflowPlan`, `WorkflowStep`, `WorkflowStepKind`, `WorkflowRisk`, `WorkflowConditionResult`.
- Produces: `StepResult`, `StepStatus`, `PreparedWorkflow`, `prepare_workflow`, `needs_confirmation`, `next_step`.

- [ ] **Step 1: Write workflow preparation tests**

Create `src/workflow_cmd.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_check_enables_conditioned_install() {
        let plan = fixture_plan();
        let results = vec![StepResult::exited("check", 1, "", "not found")];
        let prepared = prepare_workflow(plan, &results).unwrap();
        assert_eq!(prepared.status_of("install"), StepStatus::Pending);
        assert!(prepared.needs_confirmation());
    }

    #[test]
    fn passed_check_skips_conditioned_install() {
        let plan = fixture_plan();
        let results = vec![StepResult::exited("check", 0, "/usr/bin/tool", "")];
        let prepared = prepare_workflow(plan, &results).unwrap();
        assert_eq!(prepared.status_of("install"), StepStatus::Skipped);
        assert!(!prepared.needs_confirmation());
        assert_eq!(prepared.next_step().unwrap().id, "verify");
    }
}
```

Define the fixture by parsing the exact three-step JSON from Task 1 so tests exercise the real schema:

```rust
fn fixture_plan() -> WorkflowPlan {
    parse_execution_plan(THREE_STEP_WORKFLOW_JSON)
        .unwrap()
        .workflow()
        .unwrap()
}
```

Place `THREE_STEP_WORKFLOW_JSON` in the test module as the full JSON object shown in Task 1.

- [ ] **Step 2: Run the tests and confirm RED**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test workflow_cmd::tests -- --nocapture
```

Expected: compilation fails because the workflow state types and functions do not exist.

- [ ] **Step 3: Implement the minimal pure coordinator state**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus { Pending, Passed, Failed, Skipped, Cancelled }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    pub step_id: String,
    pub status: StepStatus,
    pub exit_code: i32,
    pub termination: String,
    pub stdout: String,
    pub stderr: String,
}

impl StepResult {
    pub fn exited(step_id: &str, exit_code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            step_id: step_id.to_string(),
            status: if exit_code == 0 { StepStatus::Passed } else { StepStatus::Failed },
            exit_code,
            termination: "exited".to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedWorkflow {
    pub plan: WorkflowPlan,
    pub results: Vec<StepResult>,
    statuses: indexmap::IndexMap<String, StepStatus>,
}
```

Use the already-installed `indexmap` dependency. `prepare_workflow` initializes all statuses, overlays check results, and marks a conditioned step `Skipped` when the referenced check outcome does not match. `needs_confirmation` returns true only when a pending step has risk above `ReadOnly`. `next_step` returns the first pending step in plan order.

- [ ] **Step 4: Add failure-policy and ordering tests**

Add:

```rust
#[test]
fn modification_failure_stops_later_modifications() {
    let mut workflow = prepare_workflow(fixture_plan(), &[StepResult::exited("check", 1, "", "")]).unwrap();
    workflow.record(StepResult::exited("install", 1, "", "failed")).unwrap();
    assert!(workflow.is_stopped());
    assert!(workflow.next_step().is_none());
}

#[test]
fn workflow_completes_only_after_verification_passes() {
    let mut workflow = prepare_workflow(fixture_plan(), &[StepResult::exited("check", 0, "", "")]).unwrap();
    assert!(!workflow.completed());
    workflow.record(StepResult::exited("verify", 0, "ok", "")).unwrap();
    assert!(workflow.completed());
}
```

Implement `record`, `is_stopped`, and `completed`. A failed step with `Continue` proceeds, while `Stop` or `Repair` stops. Completion requires every unskipped step to be `Passed` and at least one passed `Verify`.

- [ ] **Step 5: Run focused tests and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test workflow_cmd::tests
```

Commit:

```bash
git add src/workflow_cmd.rs src/main.rs
git commit -m "feat: prepare ordered workflows"
```

---

### Task 3: Escalate risk and confirm the complete plan

**Files:**
- Modify: `src/confirm_cmd.rs`
- Modify: `src/workflow_cmd.rs`

**Interfaces:**
- Consumes: `PreparedWorkflow`, `WorkflowRisk`, existing `classify_command_risk`.
- Produces: `effective_workflow_risk`, `render_workflow_confirmation`, `confirm_workflow`.

- [ ] **Step 1: Write risk-escalation tests**

Add to `confirm_cmd::tests`:

```rust
#[test]
fn local_risk_can_raise_but_not_lower_declared_risk() {
    assert_eq!(
        effective_workflow_risk("rm -rf /tmp/x", WorkflowRisk::ReadOnly),
        WorkflowRisk::Destructive
    );
    assert_eq!(
        effective_workflow_risk("pwd", WorkflowRisk::ChangesSystem),
        WorkflowRisk::ChangesSystem
    );
}
```

- [ ] **Step 2: Run the test and confirm RED**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test confirm_cmd::tests::local_risk_can_raise_but_not_lower_declared_risk
```

Expected: compilation fails because `effective_workflow_risk` does not exist.

- [ ] **Step 3: Extend local risk classification without changing existing labels**

Add `ChangesFiles` to `CommandRiskLevel`. Classify redirects, `tee`, `mkdir`, `touch`, `mv`, `cp`, `rm`, `chmod`, and `chown` as `ChangesFiles`; classify `sudo`, package installation, Docker service startup, `systemctl`, and `launchctl` as `ChangesSystem`; retain existing destructive patterns.

Map local levels to workflow risk and return `declared.max(local)`:

```rust
pub fn effective_workflow_risk(command: &str, declared: WorkflowRisk) -> WorkflowRisk {
    let local = match classify_command_risk(command).level() {
        CommandRiskLevel::ReadOnly => WorkflowRisk::ReadOnly,
        CommandRiskLevel::ChangesFiles => WorkflowRisk::ChangesFiles,
        CommandRiskLevel::ChangesSystem => WorkflowRisk::ChangesSystem,
        CommandRiskLevel::Destructive => WorkflowRisk::Destructive,
    };
    declared.max(local)
}
```

Keep `captures_git_changes()` true for all three modifying levels.

- [ ] **Step 4: Write confirmation-rendering tests**

Add a pure rendering test:

```rust
#[test]
fn workflow_confirmation_shows_only_pending_changes_and_verification() {
    let prepared = prepare_workflow(
        fixture_plan(),
        &[StepResult::exited("check", 0, "/usr/bin/tool", "")],
    ).unwrap();
    let text = render_workflow_confirmation(&prepared);
    assert!(text.contains("verify"));
    assert!(text.contains("skipped"));
    assert!(!text.contains("brew install tool"));
}
```

Implement bilingual plain-terminal rendering. `confirm_workflow` returns `true` immediately for a workflow with no pending modifying step. Otherwise it displays the complete prepared plan and calls the existing `read_action(&['y', 'n'], 'n', ...)`. If any effective step risk is `Destructive`, call `confirm_high_risk` after the plan confirmation.

- [ ] **Step 5: Run tests and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test confirm_cmd::tests workflow_cmd::tests
```

Commit:

```bash
git add src/confirm_cmd.rs src/workflow_cmd.rs
git commit -m "feat: confirm complete workflow plans"
```

---

### Task 4: Execute workflows and record aggregate results

**Files:**
- Modify: `src/workflow_cmd.rs`
- Modify: `src/result_cmd.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `execute_cmd::run_command_capture_controlled`, `execute_cmd::with_cwd_capture`, `confirm_workflow`, `PreparedWorkflow`.
- Produces: `WorkflowStatus`, `WorkflowRecord`, `build_workflow_session_note`, and `run_workflow_plan` in `main.rs`.

- [ ] **Step 1: Write aggregate record tests**

Add to `result_cmd::tests`:

```rust
#[test]
fn workflow_note_contains_plan_steps_and_final_status() {
    let record = workflow_record_fixture(WorkflowStatus::Completed);
    let note = build_workflow_session_note(&record);
    assert!(note.contains("Workflow status: completed"));
    assert!(note.contains("Step: check"));
    assert!(note.contains("Step: verify"));
    assert!(note.contains("Exit code: 0"));
}

#[test]
fn cancelled_workflow_note_keeps_partial_output() {
    let record = workflow_record_fixture(WorkflowStatus::Cancelled);
    let note = build_workflow_session_note(&record);
    assert!(note.contains("Workflow status: cancelled"));
    assert!(note.contains("partial output"));
}
```

Define `workflow_record_fixture(status)` in the test module with the exact three-step `WorkflowPlan` from Task 1 and these ordered results: passed `check`, skipped `install`, and passed `verify`. For `Cancelled`, replace the `verify` result with a cancelled result whose stdout is `"partial output"` and exit code is 130.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test result_cmd::tests -- --nocapture
```

Expected: compilation fails because workflow record types do not exist.

- [ ] **Step 3: Implement workflow records**

In `workflow_cmd.rs`, define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStatus { Completed, Failed, Cancelled }

#[derive(Debug, Clone)]
pub struct WorkflowRecord {
    pub request: String,
    pub plan: WorkflowPlan,
    pub results: Vec<StepResult>,
    pub repair_attempts: u8,
    pub status: WorkflowStatus,
}
```

Implement `as_str()` for both `WorkflowStatus` and `StepStatus`. In `result_cmd.rs`, use existing `truncate_for_session` for each stdout/stderr field and render one aggregate note with the original request, serialized confirmed plan, ordered results, skip reasons, repair count, and final status.

- [ ] **Step 4: Route workflow mode in main**

Add `RouteKind::Workflow` handling:

```rust
RouteKind::Workflow => {
    let workflow = plan.workflow().context("workflow payload missing")?;
    run_workflow_plan(
        config,
        shell,
        input.text(),
        workflow,
        abort_signal,
        0,
    ).await
}
```

Implement `run_workflow_plan` with this exact order:

1. Classify every `check` locally. If any effective risk is above `ReadOnly`, reject the workflow before running any check; otherwise execute pending checks.
2. Build `PreparedWorkflow` from check results.
3. Call `confirm_workflow`; if declined, append a `cancelled` workflow note without executing actions.
4. Execute each pending `action` and `verify` step with `with_cwd_capture` and `run_command_capture_controlled`.
5. Convert each `CommandOutput` to `StepResult` and record it once.
6. Stop immediately on cancellation or a failed step whose policy is `Stop` or `Repair`.
7. Append one aggregate workflow session note.
8. Return a typed outcome rather than calling `process::exit` inside the loop.

Use the existing change snapshot/report around modifying steps. Do not reuse the single-command cache for workflows in this version.

Keep command execution reusable and testable with this production helper in `workflow_cmd.rs`:

```rust
pub async fn execute_prepared_workflow(
    shell: &Shell,
    workflow: &mut PreparedWorkflow,
    abort_signal: AbortSignal,
) -> anyhow::Result<WorkflowStatus>;
```

This helper assumes confirmation has already occurred. It executes only pending non-check steps in order, records each `CommandOutput` exactly once, stops on cancellation or stop/repair failure, and returns `Completed`, `Failed`, or `Cancelled`. `main.rs` must call it only after `confirm_workflow` returns true.

- [ ] **Step 5: Add a real temporary-directory execution test**

Add a Tokio test in `workflow_cmd.rs` that runs through the public coordinator helper with the existing `Shell` and `execute_cmd` functions:

```rust
#[tokio::test]
async fn temporary_file_workflow_runs_action_then_verify() {
    let root = std::env::temp_dir().join(format!("aicmd-workflow-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("result.txt");
    let plan = temp_file_plan(&file);
    let mut workflow = prepare_workflow(plan, &[]).unwrap();
    assert!(workflow.needs_confirmation());
    let shell = Shell::new("sh", "/bin/sh", "-c");
    let status = execute_prepared_workflow(&shell, &mut workflow, create_abort_signal())
        .await
        .unwrap();
    assert_eq!(status, WorkflowStatus::Completed);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "ok\n");
    std::fs::remove_dir_all(root).unwrap();
}
```

Define `temp_file_plan(&file)` in the test module as an action `printf 'ok\n' > <quoted file>` followed by a read-only verify step `test "$(cat <quoted file>)" = ok`. Use the existing shell-quoting convention. The test bypasses interactive input only after asserting that confirmation is required; it does not bypass risk preparation or step ordering.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test workflow_cmd::tests result_cmd::tests
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
```

Commit:

```bash
git add src/workflow_cmd.rs src/result_cmd.rs src/main.rs
git commit -m "feat: execute and record workflows"
```

---

### Task 5: Generate and confirm repaired workflow plans

**Files:**
- Modify: `src/repair_cmd.rs`
- Modify: `src/workflow_cmd.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: failed `WorkflowRecord`, existing `request_execution_plan`, existing retry budget.
- Produces: `WorkflowRepairContext`, `build_workflow_repair_prompt`, and two-repair enforcement.

- [ ] **Step 1: Write repair prompt and limit tests**

Add:

```rust
#[test]
fn workflow_repair_prompt_contains_old_plan_and_failed_step() {
    let prompt = build_workflow_repair_prompt(&WorkflowRepairContext {
        user_task: "install tool",
        shell: "zsh",
        os: "macos",
        cwd: "/tmp",
        previous_plan_json: r#"{"mode":"workflow"}"#,
        completed_results: "check: passed",
        failed_step: "verify",
        exit_code: 1,
        stdout: "",
        stderr: "not found",
    });
    assert!(prompt.contains("Return a complete revised workflow plan"));
    assert!(prompt.contains("返回完整的修订 workflow 计划"));
    assert!(prompt.contains("Failed step / 失败步骤: verify"));
    assert!(prompt.contains("Previous plan / 旧计划"));
}

#[test]
fn repair_limit_is_two() {
    assert!(can_repair_workflow(0));
    assert!(can_repair_workflow(1));
    assert!(!can_repair_workflow(2));
}
```

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test repair_cmd::tests workflow_cmd::tests::repair_limit_is_two
```

Expected: compilation fails because the workflow repair APIs do not exist.

- [ ] **Step 3: Implement the bilingual repair context**

Define in `repair_cmd.rs`:

```rust
pub struct WorkflowRepairContext<'a> {
    pub user_task: &'a str,
    pub shell: &'a str,
    pub os: &'a str,
    pub cwd: &'a str,
    pub previous_plan_json: &'a str,
    pub completed_results: &'a str,
    pub failed_step: &'a str,
    pub exit_code: i32,
    pub stdout: &'a str,
    pub stderr: &'a str,
}
```

`build_workflow_repair_prompt` must demand one complete strict JSON `workflow` plan, preserve already achieved outcomes where safe, include a read-only verification step, forbid Markdown/prose, and state that no command has approval until the revised complete plan is shown again.

Add to `workflow_cmd.rs`:

```rust
pub const MAX_WORKFLOW_REPAIRS: u8 = 2;

pub fn can_repair_workflow(attempts: u8) -> bool {
    attempts < MAX_WORKFLOW_REPAIRS
}
```

- [ ] **Step 4: Connect repair generation in main**

When `run_workflow_plan` returns `Failed` with policy `Repair` and `can_repair_workflow(repair_attempts)`:

1. Build `WorkflowRepairContext` from the aggregate record.
2. Create a new planner `Input` with the repair prompt and current session context.
3. Call `request_execution_plan` with a fresh request-level `RetryBudget`.
4. Require `PlanMode::Workflow`; reject any other mode.
5. Call `run_workflow_plan` recursively with `repair_attempts + 1`.

Because each call starts from pre-checks and `confirm_workflow`, every changed plan is displayed and confirmed again. Do not execute a corrected command directly from prose or a single-command repair response.

- [ ] **Step 5: Add cancellation and no-retry tests**

Add a controlled execution test that records partial output, sets the existing abort signal, and asserts:

```rust
assert_eq!(record.status, WorkflowStatus::Cancelled);
assert_eq!(record.results[0].exit_code, 130);
assert!(record.results[0].stdout.contains("before-cancel"));
assert!(record.results.iter().all(|result| result.step_id != "later-action"));
```

Add a failure test with an action that appends one line then exits non-zero. Assert the file contains exactly one line and the action has exactly one `StepResult`, proving modification steps are not retried.

- [ ] **Step 6: Run tests and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test workflow_cmd::tests repair_cmd::tests
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
```

Commit:

```bash
git add src/repair_cmd.rs src/workflow_cmd.rs src/main.rs
git commit -m "feat: repair failed workflows safely"
```

---

### Task 6: Update terminal help and bilingual documentation

**Files:**
- Modify: `src/help_cmd.rs`
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `docs/aicmd-usage.md`

**Interfaces:**
- Consumes: implemented workflow behavior from Tasks 1-5.
- Produces: accurate user documentation and help output.

- [ ] **Step 1: Update ordinary examples**

Add this ordinary example to Chinese and English docs and `aicmd help`:

```bash
aicmd "安装 jq，并验证安装结果"
```

Explain that AICmd may automatically run read-only checks, then shows the complete change plan once. Plan changes and repair plans require confirmation again.

- [ ] **Step 2: Document safety and completion semantics**

Add matching bilingual text:

```text
Read-only checks run automatically. File and system changes run only after the complete workflow plan is confirmed. Modification steps are never retried automatically. A workflow is complete only after its read-only verification succeeds.

只读检查会自动运行。文件和系统修改只有在完整 workflow 计划确认后才会执行。修改步骤绝不自动重试；只有只读验证成功后，workflow 才算完成。
```

Document cancellation, aggregate session records, and the two-repair limit. Do not expose the JSON schema in the top-level README; keep the detailed schema in `docs/aicmd-usage.md`.

- [ ] **Step 3: Review docs with karpathy-guidelines**

Verify that every statement describes implemented behavior, Chinese and English meanings match, and the primary README still leads with `aicmd <task>` rather than a new command.

- [ ] **Step 4: Verify help and commit**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo build
target/debug/aicmd help
target/debug/aicmd help fix
git diff --check
```

Commit:

```bash
git add src/help_cmd.rs README.md README.en.md docs/aicmd-usage.md
git commit -m "docs: explain workflow execution"
```

---

### Task 7: Full regression, safe smoke tests, installation, and memory

**Files:**
- No source changes expected unless verification exposes a defect.

**Interfaces:**
- Consumes: all prior tasks.
- Produces: verified local binary and durable project record.

- [ ] **Step 1: Run the full quality gates**

Run:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --check
PATH="$HOME/.cargo/bin:$PATH" cargo test
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --all --all-targets -- -D warnings
PATH="$HOME/.cargo/bin:$PATH" cargo build --release
git diff --check
```

Expected: zero failures and zero warnings.

- [ ] **Step 2: Run strict-plan and compatibility checks**

Run focused tests:

```bash
PATH="$HOME/.cargo/bin:$PATH" cargo test plan_cmd::tests workflow_cmd::tests
PATH="$HOME/.cargo/bin:$PATH" cargo test do_cmd::tests search_cmd::tests repair_cmd::tests
```

Confirm `direct`, `script`, `search`, `diagnose`, explicit `do/search/err`, high-risk confirmation, command cancellation, and session tests remain green.

- [ ] **Step 3: Run safe local workflow smoke tests**

Use a fresh temporary directory. First run a read-only workflow that reports Rust and Git versions. Then run a file workflow whose only change is inside that temporary directory:

```text
aicmd "检查当前 Rust 和 Git 版本，并验证两个命令都可用"
aicmd "在指定临时目录创建 result.txt，写入 ok，并验证文件内容"
```

Review the complete plan before confirming the file workflow. Verify the file content, aggregate session record, and final `completed` status. Remove only the test-created temporary directory after verification.

- [ ] **Step 4: Verify failure and cancellation behavior**

Use test-only fixtures or the controlled integration tests from Task 5 to prove:

- successful action plus failed verification is `failed`;
- a revised plan is displayed again;
- modification action count remains one;
- Ctrl-C returns 130, preserves partial output, and leaves later steps unexecuted.

- [ ] **Step 5: Install and verify the local binary**

Run:

```bash
install -m 0755 target/release/aicmd "$HOME/.local/bin/aicmd"
"$HOME/.local/bin/aicmd" --version
"$HOME/.local/bin/aicmd" help
"$HOME/.local/bin/aicmd" doctor
```

- [ ] **Step 6: Save implementation evidence and report state**

Save to agentmemory with project `aicmd`: workflow contract, safety boundaries, repair limit, affected files, commit IDs, exact verification commands, test count, installed binary path, and any retained limitations.

Report local commits and preserved unrelated worktree files. Do not push GitHub unless the user explicitly asks.

---

## Plan Self-Review

- **Spec coverage:** Task 1 covers strict schema and planner output. Tasks 2-4 cover checks, conditions, confirmation, ordered execution, verification, cancellation, and session records. Task 5 covers revised plans and the two-repair limit. Task 6 covers bilingual help/docs. Task 7 covers regression, safe smoke tests, installation, and agentmemory.
- **Type consistency:** `WorkflowPlan` and step enums originate in `plan_cmd`; `PreparedWorkflow`, results, status, and records originate in `workflow_cmd`; confirmation consumes those types; result and repair modules consume records without redefining them.
- **Scope:** No new dependencies, command names, configuration keys, parallel execution, background execution, or generic agent loop.
- **Safety:** Pre-confirmation execution is limited to locally verified read-only checks. All changed plans re-enter the same full-plan confirmation path. Modification commands are never retried automatically.
