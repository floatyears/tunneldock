# Cross-platform System Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a cross-platform system tray and a close-choice workflow that either exits TunnelDock or hides it to the tray.

**Architecture:** A focused Rust module owns the close state machine, native confirmation dialog, tray menu, and main-window restoration. The existing Tauri builder wires the module into window events and retains the existing `RunEvent::ExitRequested` cleanup path for every real exit.

**Tech Stack:** Rust 2021, Tauri 2.11, tauri-plugin-dialog 2.7, Cargo unit tests

**Spec:** `docs/superpowers/specs/2026-09-18-cross-platform-system-tray.md`

## Global Constraints

- Support Windows, macOS, and Linux through Tauri's cross-platform APIs.
- Preserve the existing asynchronous process cleanup before shutdown.
- Do not add a new dependency; enable Tauri's existing `tray-icon` feature only.
- Keep frontend APIs and the custom title bar unchanged.

---

### Task 1: Close lifecycle state machine

**Files:**
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `tauri_plugin_dialog::MessageDialogResult` returned by the native close prompt.
- Produces: `CloseLifecycle::begin_close_request() -> CloseRequest`, `CloseLifecycle::finish_prompt(MessageDialogResult) -> CloseAction`, and `CloseLifecycle::request_exit()`.

- [x] **Step 1: Write failing lifecycle tests**

Add unit tests proving the first close request opens one prompt, concurrent close requests are ignored, the tray choice hides the window, cancel does nothing, and exit marks later closes as allowed.

- [x] **Step 2: Run the focused test and verify RED**

Run: `cargo test tray::tests --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because `tray` and its lifecycle types do not exist.

- [x] **Step 3: Implement the minimal lifecycle state machine**

Use two `AtomicBool` values for `prompt_open` and `exit_requested`; map the dialog's custom labels to `CloseAction::{Exit, Hide, Cancel}` and always clear the prompt guard when the dialog completes.

- [x] **Step 4: Run the focused test and verify GREEN**

Run: `cargo test tray::tests --manifest-path src-tauri/Cargo.toml`

Expected: all lifecycle tests PASS.

### Task 2: Native close prompt and tray integration

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `CloseLifecycle`, the `main` Tauri webview window, default application icon, and existing `AppState` cleanup path.
- Produces: `setup(app, lifecycle) -> tauri::Result<()>` and `handle_close_requested(window, api, lifecycle)`.

- [x] **Step 1: Write the failing decision-mapping tests**

Add literal-result tests for the custom “直接退出”, “最小化到托盘”, and “取消” dialog buttons, plus the native dialog-cancel result.

- [x] **Step 2: Run the focused tests and verify RED**

Run: `cargo test tray::tests --manifest-path src-tauri/Cargo.toml`

Expected: FAIL because the dialog-result mapping is missing.

- [x] **Step 3: Implement the native integration**

Enable Tauri's `tray-icon` feature. Build a menu containing “显示主界面”, a separator, and “退出”; restore the main window from the show item and supported left-click events; call `app.exit(0)` from the exit item; prevent native close while a non-blocking three-button dialog is open; hide on minimize and exit through `app.exit(0)` on direct exit.

- [x] **Step 4: Run focused tests and compile checks**

Run: `cargo test tray::tests --manifest-path src-tauri/Cargo.toml`

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: tests PASS and desktop integration compiles.

### Task 3: Regression verification

**Files:**
- Verify only; no planned production-file changes.

**Interfaces:**
- Consumes: completed tray implementation.
- Produces: evidence that Rust and frontend regressions were not introduced.

- [x] **Step 1: Run all Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: all tests PASS.

- [x] **Step 2: Run the frontend production build**

Run: `npm run build`

Expected: TypeScript checking and Vite production build PASS.

- [x] **Step 3: Inspect the final diff**

Run: `git diff --check` and `git diff --stat`

Expected: no whitespace errors and changes remain limited to the planned files.
