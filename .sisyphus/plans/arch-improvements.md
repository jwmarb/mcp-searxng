# Architectural Improvements: searxng-cli Rust Codebase

## TL;DR

> **Quick Summary**: Five targeted refactoring tasks to reduce code duplication, improve modularity, and centralize cross-cutting concerns in the searxng-cli Rust CLI + axum server.
> 
> **Deliverables**:
> - New `src/time.rs` module centralizing all timestamp utilities
> - Deduplicated `BrowserClient` with private helper methods
> - Unified `respond` function in routes.rs (eliminate `respond_inner`)
> - `delegate_with_history!` macro reducing SessionManager boilerplate
> - Cleaned test suite (removed duplicate truncate_content tests)
> 
> **Estimated Effort**: Medium (5 focused refactoring tasks)
> **Parallel Execution**: YES - 3 waves
> **Critical Path**: Task 1 → Task 4 (timestamp imports → macro uses same file)

---

## Context

### Original Request
Five architectural improvements to the searxng-cli Rust codebase, each targeting a specific duplication or modularity issue. All are pure refactoring — no behavior changes.

### Key Findings from Code Review

**Timestamp duplication**: `format_timestamp()` defined in `src/server/history.rs` is consumed by `src/response.rs` via `use crate::server::format_timestamp`. The module re-export chain is: `server/history.rs` → `server/mod.rs (pub use history::*)` → `crate::server::format_timestamp`. Tests for timestamp functions are duplicated across `history.rs` (lines 98-165) and `session.rs` (lines 148-191).

**BrowserClient repetition**: All 10 methods follow identical patterns: build URL → send request → check status → parse JSON or return error. The `config: Config` field is never accessed after construction (only `server_url` is used). `base64::Engine` IS used in the `screenshot()` method.

**respond vs respond_inner**: `respond()` maps 4 CliError variants to HTTP codes. `respond_inner()` maps only Http→502 and _→500. The search/fetch handlers use `respond_inner` (missing SessionNotFound, SessionRequired, ServerNotRunning mappings). Screenshot handler uses inline match.

**SessionManager delegation**: 8 methods follow identical `with_history(...)` pattern. Only 5 methods deviate: `navigate` (auto-create), `kill` (history removal), `list` (merge), `list_tabs` (no history), `pool_status` (no history).

**Test duplication**: `src/fetch/util.rs` has two full sets of `truncate_content` tests — one migrated from mod.rs (lines 63-101) and one from hybrid.rs (lines 103-139).

### Merge Conflict Analysis

| Task Pair | Shared Files | Conflict Risk |
|-----------|-------------|---------------|
| 1 ↔ 4 | `src/server/session.rs` | **HIGH** — Task 1 changes imports + moves tests; Task 4 rewrites method bodies |
| 1 ↔ 3 | None directly | LOW — Task 1 changes server/mod.rs re-exports, Task 3 uses them |
| 2 ↔ 3 | None | NONE — different files entirely |
| 2 ↔ 5 | None | NONE |
| 3 ↔ 4 | `src/server/routes.rs` uses SessionManager | NONE — routes calls, session defines |

**Resolution**: Task 1 MUST complete before Task 4. All others can run in parallel.

---

## Work Objectives

### Core Objective
Eliminate code duplication and improve module boundaries without changing any externally-observable behavior.

### Concrete Deliverables
- `src/time.rs` — new module with `format_timestamp`, `iso_timestamp`, `instant_to_iso`
- `src/browser_client.rs` — reduced from 195 lines to ~100 lines
- `src/server/routes.rs` — single `respond()` function, no `respond_inner`
- `src/server/session.rs` — `delegate_with_history!` macro reducing 8 methods to macro invocations
- `src/fetch/util.rs` — removed 37 lines of duplicate tests

### Definition of Done
- [ ] `cargo build` succeeds with no warnings
- [ ] `cargo test` passes (all existing tests, none removed except duplicates in Task 5)
- [ ] `cargo clippy` has no new warnings
- [ ] No behavior changes (same HTTP responses, same CLI output)

### Must Have
- All existing public APIs preserved (function signatures, HTTP routes)
- All tests pass (behavioral preservation proof)
- Each task compiles independently after application

### Must NOT Have (Guardrails)
- NO new dependencies added
- NO behavior changes (this is pure refactoring)
- NO changes to public API signatures
- NO removal of tests that verify unique behavior (only remove exact duplicates)
- NO over-abstraction (e.g., don't create traits just for the sake of it)
- NO changes to Cargo.toml
- Task 4 macro: NO metaprogramming that hides control flow or error handling

---

## Verification Strategy

> **ZERO HUMAN INTERVENTION** — ALL verification is agent-executed. No exceptions.

### Test Decision
- **Infrastructure exists**: YES (inline `#[cfg(test)]` modules + `tests/` directory)
- **Automated tests**: TDD approach — write/verify tests BEFORE and AFTER each refactor
- **Framework**: built-in `cargo test` (no external test framework)
- **Strategy**: For each task: `cargo test` before (baseline) → make changes → `cargo test` after (regression)

### QA Policy
Every task MUST run `cargo build && cargo test && cargo clippy` as verification.
Evidence: terminal output captured to `.sisyphus/evidence/task-{N}-cargo-check.txt`.

---

## Execution Strategy

### Parallel Execution Waves

```
Wave 1 (Start Immediately — independent tasks):
├── Task 1: Extract timestamp utilities into src/time.rs [deep]
├── Task 2: Consolidate BrowserClient methods [quick]
├── Task 3: Unify respond helpers in routes.rs [quick]
└── Task 5: Remove duplicated truncate_content tests [quick]

Wave 2 (After Task 1 — depends on new time.rs imports):
└── Task 4: SessionManager delegation macro [deep]

Wave FINAL (After ALL tasks — verification):
├── Task F1: Plan compliance audit (oracle)
├── Task F2: Code quality review (unspecified-high)
├── Task F3: Full cargo test + clippy (unspecified-high)
└── Task F4: Scope fidelity check (deep)
-> Present results -> Get explicit user okay
```

### Dependency Matrix

| Task | Depends On | Blocks | Wave |
|------|-----------|--------|------|
| 1 | — | 4 | 1 |
| 2 | — | — | 1 |
| 3 | — | — | 1 |
| 4 | 1 | — | 2 |
| 5 | — | — | 1 |

### Agent Dispatch Summary

- **Wave 1**: **4 tasks** — T1 → `deep` (new module + cross-file rewiring), T2 → `quick`, T3 → `quick`, T5 → `quick`
- **Wave 2**: **1 task** — T4 → `deep` (macro design + testing)
- **FINAL**: **4 tasks** — F1 → `oracle`, F2 → `unspecified-high`, F3 → `unspecified-high`, F4 → `deep`

---

## TODOs

- [ ] 1. Extract timestamp utilities into `src/time.rs`

  **What to do**:
  1. Create `src/time.rs` with the 3 functions cut from `src/server/history.rs`:
     - `pub fn format_timestamp(dur: Duration) -> String` (lines 31-61 of history.rs)
     - `pub fn iso_timestamp() -> String` (lines 18-21 of history.rs)
     - `pub fn instant_to_iso(instant: Instant) -> String` (lines 23-29 of history.rs)
  2. Add `use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};` to time.rs
  3. Add `#[cfg(test)] mod tests` section to `src/time.rs` with the format_timestamp tests currently in `src/server/history.rs` (lines 98-165: epoch_zero, 2023_jan_01, leap_year, year_end_boundary, with_nanos, century_non_leap) AND the iso_timestamp test (lines 129-141)
  4. In `src/server/history.rs`:
     - Remove the 3 function definitions (lines 18-61)
     - Replace with: `use crate::time::{format_timestamp, iso_timestamp, instant_to_iso};`
     - Remove the timestamp tests from `#[cfg(test)]` (lines 98-165), keep `HistoryEntry` serialization test and `with_history_*` async tests
     - Remove `SystemTime, UNIX_EPOCH` from the `use std::time::{}` import (keep `Duration, Instant`)
     - Actually: history.rs no longer needs `Duration` or `SystemTime` or `UNIX_EPOCH` since those were only for the timestamp functions. It still needs `Instant` for `with_history`. Keep `use std::time::Instant;`
  5. In `src/response.rs` line 5: change `use crate::server::format_timestamp;` to `use crate::time::format_timestamp;`
  6. In `src/response.rs` line 2: keep `use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};` (it uses these directly)
  7. In `src/server/session.rs` line 10: change `use super::history::{with_history, HistoryEntry, instant_to_iso, iso_timestamp};` to:
     - `use super::history::{with_history, HistoryEntry};`
     - `use crate::time::{instant_to_iso, iso_timestamp};`
  8. In `src/server/session.rs` tests (lines 142-241): Remove the duplicate `format_timestamp` tests (lines 148-191: test_format_timestamp_epoch_zero through test_iso_timestamp_valid_format). Keep the SessionInfo/HistoryEntry serialization tests (lines 193-240).
  9. In `src/server/session.rs` tests: Remove `use crate::server::history;` and `use std::time::Duration;` imports (no longer needed after removing timestamp tests).
  10. In `src/lib.rs`: Add `pub mod time;` (after `pub mod response;` or at end)
  11. In `src/main.rs`: Add `mod time;` (after `mod response;` or at end)

  **Must NOT do**:
  - Do NOT change the function implementations (same algorithm, no chrono)
  - Do NOT remove the `with_history` async tests from history.rs
  - Do NOT change `src/server/mod.rs` (the `pub use history::*` no longer re-exports timestamp fns, which is the desired outcome — response.rs now imports directly from `crate::time`)

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Cross-file refactoring touching 6 files with import chain analysis
  - **Skills**: [`rust-style`]
    - `rust-style`: Module organization, import conventions, idiomatic Rust patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 2, 3, 5)
  - **Parallel Group**: Wave 1 (with Tasks 2, 3, 5)
  - **Blocks**: Task 4 (macro needs the new import paths)
  - **Blocked By**: None (can start immediately)

  **References**:

  **Pattern References** (existing code to follow):
  - `src/server/history.rs:18-61` — The 3 functions to MOVE (cut from here, paste to time.rs)
  - `src/server/history.rs:94-165` — Tests to MOVE to time.rs (format_timestamp tests)
  - `src/server/history.rs:167-248` — Tests to KEEP in history.rs (with_history async tests)

  **API/Type References** (contracts to preserve):
  - `src/response.rs:5` — Current import: `use crate::server::format_timestamp;` → must become `use crate::time::format_timestamp;`
  - `src/server/session.rs:10` — Current import line to split

  **Test References** (what to keep vs remove):
  - `src/server/session.rs:148-191` — REMOVE (duplicates of history.rs timestamp tests)
  - `src/server/session.rs:193-240` — KEEP (SessionInfo + HistoryEntry serialization)

  **WHY Each Reference Matters**:
  - The history.rs functions are the SOURCE to move — executor must cut exactly these lines
  - The response.rs import is the critical downstream consumer — breaking this breaks compilation
  - The session.rs duplicate tests are what makes this refactoring net-positive (removes ~50 lines of duplication)

  **Acceptance Criteria**:

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Compilation succeeds after timestamp extraction
    Tool: Bash
    Preconditions: All file edits applied
    Steps:
      1. Run `cargo build 2>&1`
      2. Assert exit code 0
      3. Assert output contains "Finished"
      4. Assert no "unresolved import" errors
    Expected Result: Clean compilation with no errors or warnings about imports
    Failure Indicators: "cannot find" or "unresolved import" in stderr
    Evidence: .sisyphus/evidence/task-1-build.txt

  Scenario: All timestamp tests pass from new location
    Tool: Bash
    Preconditions: src/time.rs exists with tests
    Steps:
      1. Run `cargo test time:: 2>&1`
      2. Assert all format_timestamp tests pass
      3. Assert iso_timestamp test passes
      4. Count test results: should be 7 tests (6 format + 1 iso)
    Expected Result: "test result: ok. 7 passed; 0 failed"
    Failure Indicators: Any test failure or "0 passed"
    Evidence: .sisyphus/evidence/task-1-time-tests.txt

  Scenario: History and session tests still pass
    Tool: Bash
    Preconditions: Duplicate tests removed from session.rs
    Steps:
      1. Run `cargo test server::history:: 2>&1`
      2. Run `cargo test server::session:: 2>&1`
      3. Assert with_history async tests pass (4 tests)
      4. Assert SessionInfo serialization tests pass
    Expected Result: All remaining tests pass, no timestamp tests in session module
    Failure Indicators: "FAILED" or test count unexpectedly high
    Evidence: .sisyphus/evidence/task-1-history-session-tests.txt

  Scenario: No duplicate timestamp functions remain
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. Run `grep -rn "fn format_timestamp" src/`
      2. Assert exactly 1 match: `src/time.rs`
      3. Run `grep -rn "fn iso_timestamp" src/`
      4. Assert exactly 1 match: `src/time.rs`
      5. Run `grep -rn "fn instant_to_iso" src/`
      6. Assert exactly 1 match: `src/time.rs`
    Expected Result: Each function defined exactly once in src/time.rs
    Failure Indicators: Multiple matches or match in src/server/history.rs
    Evidence: .sisyphus/evidence/task-1-no-duplicates.txt
  ```

  **Commit**: YES
  - Message: `refactor(time): extract timestamp utilities into src/time.rs`
  - Files: `src/time.rs`, `src/lib.rs`, `src/main.rs`, `src/server/history.rs`, `src/server/session.rs`, `src/response.rs`
  - Pre-commit: `cargo test`

- [ ] 2. Consolidate BrowserClient methods with private helpers

  **What to do**:
  1. In `src/browser_client.rs`, add 3 private helper methods to `impl BrowserClient`:
     ```rust
     async fn post_unit(&self, path: &str, body: &serde_json::Value) -> Result<()> {
         let response = self.client
             .post(format!("{}{}", self.server_url, path))
             .json(body)
             .send()
             .await?;
         if response.status().is_success() {
             Ok(())
         } else {
             Err(CliError::Browser(format!("Server error: {}", response.status())))
         }
     }

     async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
         let response = self.client
             .post(format!("{}{}", self.server_url, path))
             .json(body)
             .send()
             .await?;
         if response.status().is_success() {
             Ok(response.json().await?)
         } else {
             Err(CliError::Browser(format!("Server error: {}", response.status())))
         }
     }

     async fn get_json(&self, path: &str) -> Result<serde_json::Value> {
         let response = self.client
             .get(format!("{}{}", self.server_url, path))
             .send()
             .await?;
         if response.status().is_success() {
             Ok(response.json().await?)
         } else {
             Err(CliError::Browser(format!("Server error: {}", response.status())))
         }
     }
     ```
  2. Rewrite each pub method to use the helpers:
     - `navigate(id, url)` → `self.post_unit("/api/navigate", &json!({"session": id, "url": url})).await`
     - `click(id, selector)` → `self.post_unit("/api/click", &json!({"session": id, "selector": selector})).await`
     - `fill(id, selector, value)` → `self.post_unit("/api/fill", &json!({"session": id, "selector": selector, "value": value})).await`
     - `kill(id)` → `self.post_unit("/api/kill", &json!({"session": id})).await`
     - `snapshot(id)` → `let body = self.get_json(&format!("/api/snapshot?session={}", id)).await?; Ok(body["data"].as_str().unwrap_or("").to_string())`
     - `evaluate(id, script)` → `let body = self.post_json("/api/evaluate", &json!({"session": id, "script": script})).await?; Ok(body["data"].clone())`
     - `tabs(id, action, url)` → build payload, `self.post_json("/api/tabs", &payload).await`
     - `instances()` → `self.get_json("/api/instances").await`
     - `session_info(id)` → `self.get_json(&format!("/api/session/{}?info=true", id)).await`
     - `screenshot(id, file_path)` → Keep inline (needs raw bytes, not JSON)
  3. Remove the `config: Config` field from the struct. Change `new()` to:
     ```rust
     pub fn new(config: &Config) -> Self {
         Self {
             client: Client::new(),
             server_url: config.browser_server_url.clone(),
         }
     }
     ```
  4. Remove `use crate::config::Config;` from the imports — wait, `new()` still takes `&Config` parameter. Keep the import.
  5. Keep `use base64::Engine;` (still used by `screenshot`)
  6. Update the test `test_browser_client_new` — it accesses `client.server_url` which remains valid. But the `config` field check (if any) would fail. Looking at the tests: they only check `client.server_url`, so they'll pass as-is.

  **Must NOT do**:
  - Do NOT change `screenshot()` to use helpers (it needs raw bytes)
  - Do NOT change the public API (same method signatures)
  - Do NOT add generic type parameters to helpers (keep it simple with `serde_json::Value`)
  - Do NOT remove `base64::Engine` import (screenshot uses it)

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Single-file change, mechanical refactoring, well-defined pattern
  - **Skills**: [`rust-style`]
    - `rust-style`: Idiomatic Rust method delegation patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 1, 3, 5)
  - **Parallel Group**: Wave 1
  - **Blocks**: None
  - **Blocked By**: None

  **References**:

  **Pattern References** (existing code to follow):
  - `src/browser_client.rs:21-36` — navigate() pattern showing the repetitive structure to consolidate
  - `src/browser_client.rs:105-124` — screenshot() is the EXCEPTION that must stay inline (raw bytes)

  **API/Type References** (contracts to preserve):
  - `src/browser_client.rs:6-10` — BrowserClient struct definition (remove `config` field)
  - `src/browser_client.rs:12-19` — `new()` constructor (simplify)
  - `src/browser_client.rs:197-217` — Tests that must still pass after changes

  **WHY Each Reference Matters**:
  - The navigate pattern shows the exact duplication (5 methods are identical to this pattern)
  - The screenshot exception ensures executor doesn't over-consolidate
  - The tests prove the public API contract that must be preserved

  **Acceptance Criteria**:

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Compilation succeeds after BrowserClient consolidation
    Tool: Bash
    Preconditions: All helper methods added, pub methods rewritten
    Steps:
      1. Run `cargo build 2>&1`
      2. Assert exit code 0
      3. Assert no "unused" warnings for the 3 helper methods
    Expected Result: Clean build
    Failure Indicators: "dead_code" warnings, compilation errors
    Evidence: .sisyphus/evidence/task-2-build.txt

  Scenario: Existing tests pass unchanged
    Tool: Bash
    Preconditions: Struct field removed, methods consolidated
    Steps:
      1. Run `cargo test browser_client:: 2>&1`
      2. Assert test_browser_client_new passes
      3. Assert test_browser_client_custom_port passes
    Expected Result: "test result: ok. 2 passed; 0 failed"
    Failure Indicators: Compilation error in tests, field access failure
    Evidence: .sisyphus/evidence/task-2-tests.txt

  Scenario: Line count reduced significantly
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. Run `wc -l src/browser_client.rs`
      2. Assert line count is < 150 (was 217)
      3. Run `grep -c "pub async fn" src/browser_client.rs`
      4. Assert still 10 public methods (unchanged API surface)
    Expected Result: ~100-140 lines, 10 pub methods preserved
    Failure Indicators: >150 lines (insufficient consolidation) or ≠10 pub methods
    Evidence: .sisyphus/evidence/task-2-metrics.txt
  ```

  **Commit**: YES
  - Message: `refactor(browser_client): consolidate with private helpers`
  - Files: `src/browser_client.rs`
  - Pre-commit: `cargo test`

- [ ] 3. Unify respond helpers in routes.rs

  **What to do**:
  1. In `src/server/routes.rs`, delete `respond_inner` (lines 131-143)
  2. Change `api_search` (line 99) from `respond_inner(search.search(...).await)` to `respond(search.search(...).await)`
  3. Change `api_fetch` (line 128) from `respond_inner(fetcher.fetch(...).await)` to `respond(fetcher.fetch(...).await)`
  4. That's it. The existing `respond()` function (lines 229-244) already handles all error variants including `Http → BAD_GATEWAY` and the fallback `_ → INTERNAL_SERVER_ERROR`. The search/fetch handlers will now get proper error mapping for SessionNotFound/SessionRequired/ServerNotRunning — these won't be triggered by search/fetch operations, so it's a no-op improvement for correctness.
  5. Leave the screenshot handler as-is (lines 299-310) — it returns binary data on success, not JSON, so it cannot use `respond()`. The inline match there is the correct approach for binary responses.

  **Must NOT do**:
  - Do NOT touch the screenshot handler (binary response needs inline match)
  - Do NOT modify `respond()` itself (it's already correct)
  - Do NOT change `bad_request_response()` (different concern: input validation before execution)
  - Do NOT remove the `Instant::now()` inside respond — it's fine for measuring response envelope time

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: 2-line change + 13-line deletion. Minimal risk.
  - **Skills**: [`rust-style`]
    - `rust-style`: Understanding of axum response patterns

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 1, 2, 5)
  - **Parallel Group**: Wave 1
  - **Blocks**: None
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/server/routes.rs:229-244` — The `respond()` function to KEEP (full error mapping)
  - `src/server/routes.rs:131-143` — The `respond_inner()` function to DELETE

  **API/Type References**:
  - `src/server/routes.rs:99` — `respond_inner(search.search(...))` → change to `respond(...)`
  - `src/server/routes.rs:128` — `respond_inner(fetcher.fetch(...))` → change to `respond(...)`
  - `src/server/routes.rs:299-310` — Screenshot handler: DO NOT TOUCH

  **Test References**:
  - `src/server/routes.rs:451-505` — Tests for `respond()` already exist and cover all error variants

  **WHY Each Reference Matters**:
  - The respond function shows the complete error mapping the search/fetch handlers will inherit
  - The respond_inner is what to delete — executor must verify no other callers exist
  - The screenshot handler is the exception boundary — executor must understand WHY it's different

  **Acceptance Criteria**:

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: respond_inner fully removed
    Tool: Bash
    Preconditions: Both callers switched to respond()
    Steps:
      1. Run `grep -n "respond_inner" src/server/routes.rs`
      2. Assert no matches (function definition and all calls removed)
      3. Run `cargo build 2>&1`
      4. Assert clean compilation
    Expected Result: Zero occurrences of "respond_inner" in routes.rs
    Failure Indicators: Any match for "respond_inner" or compilation error
    Evidence: .sisyphus/evidence/task-3-no-respond-inner.txt

  Scenario: All existing route tests pass
    Tool: Bash
    Preconditions: respond_inner deleted
    Steps:
      1. Run `cargo test server::routes:: 2>&1`
      2. Assert all tests pass (test_respond_success, test_respond_session_not_found, etc.)
    Expected Result: "test result: ok. N passed; 0 failed" where N >= 13
    Failure Indicators: Any test failure
    Evidence: .sisyphus/evidence/task-3-tests.txt

  Scenario: Screenshot handler unchanged
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. Run `grep -A 8 "async fn screenshot" src/server/routes.rs`
      2. Assert it still uses inline match (not respond())
      3. Assert "Content-Type", "image/png" still present
    Expected Result: Screenshot handler has inline match returning binary
    Failure Indicators: screenshot handler calls respond() or missing Content-Type
    Evidence: .sisyphus/evidence/task-3-screenshot-preserved.txt
  ```

  **Commit**: YES
  - Message: `refactor(routes): unify respond helpers, remove respond_inner`
  - Files: `src/server/routes.rs`
  - Pre-commit: `cargo test`

- [ ] 4. SessionManager delegation macro

  **What to do**:
  1. In `src/server/session.rs`, define a `macro_rules! delegate_with_history` at the top of the file (after imports, before the struct):
     ```rust
     /// Generate a SessionManager method that delegates to the pool with automatic history recording.
     macro_rules! delegate_with_history {
         // Variant: no extra args beyond session id, returns T
         ($method:ident, $command:expr, $pool_method:ident -> $ret:ty) => {
             pub async fn $method(&self, id: &str) -> Result<$ret> {
                 with_history(&self.history, id, $command, "", || self.pool.$pool_method(id)).await
             }
         };
         // Variant: one string arg (detail = the arg value)
         ($method:ident, $command:expr, $pool_method:ident, $arg:ident : &str -> $ret:ty) => {
             pub async fn $method(&self, id: &str, $arg: &str) -> Result<$ret> {
                 with_history(&self.history, id, $command, $arg, || self.pool.$pool_method(id, $arg)).await
             }
         };
         // Variant: two string args (detail = formatted)
         ($method:ident, $command:expr, $pool_method:ident, $a1:ident : &str, $a2:ident : &str -> $ret:ty) => {
             pub async fn $method(&self, id: &str, $a1: &str, $a2: &str) -> Result<$ret> {
                 let detail = format!("{} = {}", $a1, $a2);
                 with_history(&self.history, id, $command, &detail, || self.pool.$pool_method(id, $a1, $a2)).await
             }
         };
         // Variant: one usize arg (detail = to_string)
         ($method:ident, $command:expr, $pool_method:ident, $arg:ident : usize -> $ret:ty) => {
             pub async fn $method(&self, id: &str, $arg: usize) -> Result<$ret> {
                 with_history(&self.history, id, $command, &$arg.to_string(), || self.pool.$pool_method(id, $arg)).await
             }
         };
         // Variant: one Option<&str> arg
         ($method:ident, $command:expr, $pool_method:ident, $arg:ident : Option<&str> -> $ret:ty) => {
             pub async fn $method(&self, id: &str, $arg: Option<&str>) -> Result<$ret> {
                 let detail = $arg.unwrap_or_default();
                 with_history(&self.history, id, $command, detail, || self.pool.$pool_method(id, $arg)).await
             }
         };
     }
     ```
  2. Inside `impl SessionManager`, replace the 8 repetitive methods with macro invocations:
     ```rust
     delegate_with_history!(snapshot, "snapshot", snapshot -> String);
     delegate_with_history!(click, "click", click, selector: &str -> ());
     delegate_with_history!(fill, "fill", fill, selector: &str, value: &str -> ());
     delegate_with_history!(evaluate, "evaluate", evaluate, script: &str -> serde_json::Value);
     delegate_with_history!(screenshot, "screenshot", screenshot -> Vec<u8>);
     delegate_with_history!(new_tab, "new_tab", new_tab, url: Option<&str> -> ());
     delegate_with_history!(close_tab, "close_tab", close_tab, index: usize -> ());
     delegate_with_history!(select_tab, "select_tab", select_tab, index: usize -> ());
     ```
  3. Keep explicit implementations for:
     - `navigate` — has auto-create logic + manual history recording
     - `kill` — removes history entry
     - `list` — complex merge of pool state + history
     - `list_tabs` — no history recording
     - `pool_status` — no history recording
  4. Verify the macro generates identical signatures to the current implementations by running `cargo test`.

  **Must NOT do**:
  - Do NOT make the macro overly generic (proc-macro, trait-based abstraction)
  - Do NOT change the public method signatures
  - Do NOT touch navigate/kill/list/list_tabs/pool_status
  - Do NOT move the macro to a separate file (keep it local to session.rs)
  - Do NOT use `paste!` crate or any new dependencies

  **Recommended Agent Profile**:
  - **Category**: `deep`
    - Reason: Macro design requires careful pattern matching across argument variants
  - **Skills**: [`rust-style`]
    - `rust-style`: Macro design patterns, declarative macro best practices

  **Parallelization**:
  - **Can Run In Parallel**: NO
  - **Parallel Group**: Wave 2 (solo)
  - **Blocks**: None
  - **Blocked By**: Task 1 (imports of `instant_to_iso` and `iso_timestamp` will have changed)

  **References**:

  **Pattern References** (existing code — what macro replaces):
  - `src/server/session.rs:53-54` — `snapshot()` — simplest form (no extra args)
  - `src/server/session.rs:57-58` — `click()` — one &str arg
  - `src/server/session.rs:61-63` — `fill()` — two &str args with formatted detail
  - `src/server/session.rs:79-80` — `close_tab()` — one usize arg
  - `src/server/session.rs:74-76` — `new_tab()` — Option<&str> arg

  **API/Type References** (what macro must produce):
  - `src/server/session.rs:36-51` — `navigate()` — KEEP AS-IS (exception)
  - `src/server/session.rs:91-98` — `kill()` — KEEP AS-IS (exception)
  - `src/browser/pool.rs` — BrowserPoolHandle method signatures (macro calls these)

  **WHY Each Reference Matters**:
  - The 5 pattern examples show the FULL variety of argument shapes the macro must handle
  - The exceptions (navigate, kill) show what NOT to macroify
  - The pool methods confirm the delegation target signatures match

  **Acceptance Criteria**:

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Macro compiles and produces correct signatures
    Tool: Bash
    Preconditions: Macro defined and 8 invocations replace explicit methods
    Steps:
      1. Run `cargo build 2>&1`
      2. Assert clean compilation
      3. Run `cargo test server::session:: 2>&1`
      4. Assert all tests pass (serialization tests, etc.)
    Expected Result: Identical behavior, all tests pass
    Failure Indicators: "expected" type mismatches, "cannot find" pool methods
    Evidence: .sisyphus/evidence/task-4-build-tests.txt

  Scenario: Macro reduces line count
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. Run `wc -l src/server/session.rs`
      2. Assert line count < 180 (was 241, minus moved tests from Task 1 ~50 lines, minus 8 method bodies ~30 lines + macro ~40 lines = net reduction)
    Expected Result: <180 lines
    Failure Indicators: >180 lines (insufficient reduction) 
    Evidence: .sisyphus/evidence/task-4-metrics.txt

  Scenario: All routes still work through SessionManager
    Tool: Bash
    Preconditions: Macro-generated methods exist
    Steps:
      1. Run `cargo build 2>&1` (confirms routes.rs can still call SessionManager methods)
      2. Run `cargo test 2>&1` (full suite including integration)
      3. Assert 0 failures
    Expected Result: Full test suite passes
    Failure Indicators: Any test failure mentioning session/pool/route
    Evidence: .sisyphus/evidence/task-4-full-suite.txt

  Scenario: Exception methods remain explicit
    Tool: Bash
    Preconditions: Refactoring complete
    Steps:
      1. Run `grep -n "pub async fn navigate\|pub async fn kill\|pub async fn list\|pub async fn list_tabs\|pub async fn pool_status" src/server/session.rs`
      2. Assert 5 matches (these should be explicit, not macro-generated)
      3. Run `grep -c "delegate_with_history!" src/server/session.rs`
      4. Assert 8 macro invocations
    Expected Result: 5 explicit + 8 macro-generated = 13 total methods
    Failure Indicators: <5 explicit or ≠8 macro invocations
    Evidence: .sisyphus/evidence/task-4-method-count.txt
  ```

  **Commit**: YES
  - Message: `refactor(session): add delegate_with_history macro`
  - Files: `src/server/session.rs`
  - Pre-commit: `cargo test`

- [ ] 5. Remove duplicated truncate_content tests

  **What to do**:
  1. In `src/fetch/util.rs`, remove the second set of truncate_content tests (lines 103-139):
     - Delete the comment: `// truncate_content tests (from hybrid.rs)`
     - Delete 6 test functions: `test_truncate_content_short`, `test_truncate_content_exact`, `test_truncate_content_over`, `test_truncate_content_empty`, `test_truncate_content_zero_max`, `test_truncate_content_unicode_boundary`
  2. Keep the first set (lines 63-101): `test_truncate_short_content_unchanged`, `test_truncate_exact_boundary_unchanged`, `test_truncate_one_char_over`, `test_truncate_empty_string`, `test_truncate_unicode_at_boundary`, `test_truncate_zero_max_chars`
  3. Verify no test coverage is lost — both sets test the same behaviors:
     - short content → unchanged ✓ (both test this)
     - exact boundary → unchanged ✓ (both test this)
     - over limit → truncated + "..." ✓ (both test this)
     - empty → empty ✓ (both test this)
     - zero max → "..." ✓ (both test this)
     - unicode boundary → safe truncation ✓ (both test this)

  **Must NOT do**:
  - Do NOT remove the first set of tests (they have better names and more thorough assertions)
  - Do NOT modify any non-test code
  - Do NOT touch `extract_title` tests

  **Recommended Agent Profile**:
  - **Category**: `quick`
    - Reason: Delete 37 lines from a single file. Trivial.
  - **Skills**: [`rust-style`]
    - `rust-style`: Test organization conventions

  **Parallelization**:
  - **Can Run In Parallel**: YES (with Tasks 1, 2, 3)
  - **Parallel Group**: Wave 1
  - **Blocks**: None
  - **Blocked By**: None

  **References**:

  **Pattern References**:
  - `src/fetch/util.rs:63-101` — First set to KEEP (from mod.rs, better names)
  - `src/fetch/util.rs:103-139` — Second set to DELETE (from hybrid.rs, duplicates)

  **WHY Each Reference Matters**:
  - Line numbers are exact — executor can verify before/after by counting
  - The comment "from hybrid.rs" confirms these were migrated copies, not intentional variants

  **Acceptance Criteria**:

  **QA Scenarios (MANDATORY):**

  ```
  Scenario: Duplicate tests removed, originals intact
    Tool: Bash
    Preconditions: Second test set deleted
    Steps:
      1. Run `grep -c "fn test_truncate" src/fetch/util.rs`
      2. Assert exactly 6 (the first set)
      3. Run `grep "from hybrid.rs" src/fetch/util.rs`
      4. Assert no matches (comment deleted)
    Expected Result: 6 truncate tests remain, no "from hybrid.rs" comment
    Failure Indicators: >6 or <6 test functions, or comment still present
    Evidence: .sisyphus/evidence/task-5-test-count.txt

  Scenario: Remaining tests all pass
    Tool: Bash
    Preconditions: Deletion complete
    Steps:
      1. Run `cargo test fetch::util:: 2>&1`
      2. Assert all tests pass (6 extract_title + 6 truncate = 12 total)
    Expected Result: "test result: ok. 12 passed; 0 failed"
    Failure Indicators: Any test failure or unexpected count
    Evidence: .sisyphus/evidence/task-5-tests.txt

  Scenario: File line count reduced
    Tool: Bash
    Preconditions: Deletion complete
    Steps:
      1. Run `wc -l src/fetch/util.rs`
      2. Assert ~102 lines (was 140, removed 37 lines + closing brace alignment)
    Expected Result: ≤103 lines
    Failure Indicators: >103 lines (not enough removed)
    Evidence: .sisyphus/evidence/task-5-metrics.txt
  ```

  **Commit**: YES
  - Message: `refactor(tests): remove duplicate truncate_content tests`
  - Files: `src/fetch/util.rs`
  - Pre-commit: `cargo test`

---

## Final Verification Wave

> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.

- [ ] F1. **Plan Compliance Audit** — `oracle`
  Read the plan end-to-end. For each "Must Have": verify implementation exists (run `cargo test`). For each "Must NOT Have": search codebase for forbidden patterns (new deps in Cargo.toml, behavior changes, removed non-duplicate tests). Check evidence files exist in `.sisyphus/evidence/`. Compare deliverables against plan.
  Output: `Must Have [N/N] | Must NOT Have [N/N] | Tasks [5/5] | VERDICT: APPROVE/REJECT`

- [ ] F2. **Code Quality Review** — `unspecified-high`
  Run `cargo clippy -- -D warnings`. Review all changed files for: unused imports, dead code, overly-complex macros, missing documentation on public items. Check the macro in Task 4 compiles cleanly with no warnings.
  Output: `Clippy [PASS/FAIL] | Build [PASS/FAIL] | Tests [N pass/N fail] | VERDICT`

- [ ] F3. **Full Test Suite + Integration** — `unspecified-high`
  Run `cargo test` from clean state. Verify test count hasn't decreased unexpectedly (only the 6 duplicate tests from Task 5 should be removed). Run `cargo build --release` to verify optimized build.
  Output: `Tests [N/N pass] | Release Build [PASS/FAIL] | Test Count Delta [-6 expected] | VERDICT`

- [ ] F4. **Scope Fidelity Check** — `deep`
  For each task: read "What to do", read actual git diff. Verify 1:1 — everything in spec was built, nothing beyond spec was built. Check no public API signatures changed. Verify `response.rs` still produces identical JSON output. Verify all HTTP routes still work identically.
  Output: `Tasks [5/5 compliant] | API Unchanged [YES/NO] | Unaccounted Changes [CLEAN/N files] | VERDICT`

---

## Commit Strategy

| Commit | Task | Message | Files | Pre-commit Check |
|--------|------|---------|-------|-----------------|
| 1 | 5 | `refactor(tests): remove duplicate truncate_content tests` | `src/fetch/util.rs` | `cargo test` |
| 2 | 1 | `refactor(time): extract timestamp utilities into src/time.rs` | `src/time.rs`, `src/lib.rs`, `src/main.rs`, `src/server/history.rs`, `src/server/session.rs`, `src/response.rs` | `cargo test` |
| 3 | 2 | `refactor(browser_client): consolidate with private helpers` | `src/browser_client.rs` | `cargo test` |
| 4 | 3 | `refactor(routes): unify respond helpers, remove respond_inner` | `src/server/routes.rs` | `cargo test` |
| 5 | 4 | `refactor(session): add delegate_with_history macro` | `src/server/session.rs` | `cargo test` |

**Rationale for commit order**: Task 5 first (smallest, safest). Task 1 second (foundation for Task 4). Tasks 2+3 in any order (independent). Task 4 last (depends on Task 1's import changes).

---

## Success Criteria

### Verification Commands
```bash
cargo build              # Expected: Compiling searxng-cli... Finished
cargo test               # Expected: test result: ok. N passed; 0 failed
cargo clippy -- -D warnings  # Expected: no warnings
cargo build --release    # Expected: Finished `release` profile
```

### Final Checklist
- [ ] All "Must Have" present — public APIs preserved, all tests pass
- [ ] All "Must NOT Have" absent — no new deps, no behavior changes, no over-abstraction
- [ ] `src/time.rs` exists with 3 functions + comprehensive tests
- [ ] `BrowserClient` has ≤3 private helpers, each pub method is ≤5 lines
- [ ] `respond_inner` deleted from routes.rs
- [ ] `delegate_with_history!` macro handles 8 SessionManager methods
- [ ] Exactly 6 test functions removed from `src/fetch/util.rs`
- [ ] Total test count decreased by exactly 6 (duplicate removal) + increased by 0-2 (possible macro tests)
