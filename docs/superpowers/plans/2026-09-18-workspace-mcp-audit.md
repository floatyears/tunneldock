# Workspace MCP Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the fixed two-record demo with real per-workspace Pi tool-call auditing and MCP payload token statistics.

**Architecture:** Parse Pi JSONL output through a small pure Rust parser, then let the workspace process owner apply parsed events to persisted application history. Each record owns its workspace identity and token counts, so the existing React view can filter and aggregate without introducing another persistence model.

**Tech Stack:** Rust 2021, Tauri 2, serde_json, tiktoken-rs, React 19, TypeScript, Tailwind CSS.

**Spec:** `docs/superpowers/specs/2026-09-18-workspace-mcp-audit.md`

## Global Constraints

- Preserve existing public commands and persisted history compatibility.
- Do not modify the unrelated tray implementation already present in the working tree.
- Count MCP payload tokens with `o200k_base`; do not label them as provider billing usage.
- Follow red-green TDD for parser, persistence compatibility, and frontend aggregation behavior.

---

### Task 1: Pi RPC audit parser

**Files:**
- Create: `src-tauri/src/audit.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/audit.rs`

**Interfaces:**
- Consumes: one Pi RPC JSONL line.
- Produces: `RpcAuditEvent::{ToolStarted, ToolFinished}` with call identity, full serialized payloads, summary, status, and exact `o200k_base` token counts.

- [x] **Step 1: Write failing parser tests**

Add tests for start parsing, success/error end parsing, full-result token counting, and unrelated-line rejection.

- [x] **Step 2: Run tests to verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml audit::tests --lib`

Expected: compilation fails because the parser module and API do not exist.

- [x] **Step 3: Implement the minimal parser**

Deserialize only the documented Pi event fields, serialize arguments/results once, build a bounded result summary, and count the full payload through a cached `o200k_base` encoder.

- [x] **Step 4: Run tests to verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml audit::tests --lib`

Expected: all parser tests pass.

### Task 2: Persist real calls per workspace

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/commands/history.rs`
- Modify: `src-tauri/src/commands/workspace.rs`
- Test: `src-tauri/src/state.rs`
- Test: `src-tauri/src/commands/history.rs`

**Interfaces:**
- Consumes: `RpcAuditEvent` from Task 1 and the workspace ID/name/session ID owned by the running process.
- Produces: backward-compatible `McpCallRecord` entries and the `audit-updated` Tauri event.

- [x] **Step 1: Write failing compatibility and empty-history tests**

Assert that old JSON records default `workspace_id` and token fields, and that empty history remains empty.

- [x] **Step 2: Run tests to verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib history state`

Expected: tests fail because the fields/default behavior are missing.

- [x] **Step 3: Implement record lifecycle**

Add serde defaults, remove sample seeding, track pending tool calls inside each stdout reader, insert on start, update on end, persist after every transition, and emit `audit-updated`.

- [x] **Step 4: Run tests to verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`

Expected: all Rust library tests pass.

### Task 3: Workspace and Token statistics UI

**Files:**
- Create: `src/views/historyStats.ts`
- Create: `src/views/historyStats.test.ts`
- Modify: `src/types/index.ts`
- Modify: `src/views/HistoryView.tsx`
- Modify: `src/App.tsx`
- Modify: `package.json`

**Interfaces:**
- Consumes: backward-compatible `McpCallRecord[]` and selected workspace ID.
- Produces: filtered records plus total/read/bash/write/success/duration/input/output/total Token metrics.

- [x] **Step 1: Write failing aggregation tests**

Use literal fixtures for all-workspace and single-workspace totals, including legacy records with absent token fields.

- [x] **Step 2: Run tests to verify RED**

Run: `npm test -- --run src/views/historyStats.test.ts`

Expected: the test command or module fails before the helper exists.

- [x] **Step 3: Implement minimal aggregation and UI**

Add Vitest, implement one-pass aggregation, add the workspace selector and three token cards, and listen for `audit-updated` in `App` to reload history.

- [x] **Step 4: Run tests and builds to verify GREEN**

Run: `npm test -- --run src/views/historyStats.test.ts`

Run: `npm run build`

Expected: tests and build pass.

### Task 4: Final verification

**Files:**
- Review all files changed by Tasks 1-3.

**Interfaces:**
- Consumes: completed implementation.
- Produces: fresh verification evidence and a concise risk note for manual Chappie testing.

- [x] **Step 1: Run all automated checks**

Run: `npm test -- --run`

Run: `npm run build`

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

- [x] **Step 2: Review the final diff**

Confirm that no sample records remain, every new persisted field has a serde default, only audit-related files plus plan/spec changed, and unrelated tray changes are untouched.
