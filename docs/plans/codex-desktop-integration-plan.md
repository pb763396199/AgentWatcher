# AgentWatcher Codex Desktop Integration Plan

## Summary

Add Codex Desktop as a first-class AgentWatcher provider for monitoring, handoff, and AgentTask dispatch. The verified Windows path is a short-lived or managed local Codex app-server over WebSocket, not `codex://threads/new`, because the deep link opens or pre-fills a composer but did not create a searchable thread by itself.

## Verified Interfaces

- `codex app-server --listen ws://127.0.0.1:<port>` works on Windows when using an available loopback port. `/readyz` returns OK.
- JSON-RPC methods verified: `initialize`, `thread/list`, `thread/turns/list`, `thread/start`, `turn/start`.
- `thread/list` uses snake-case sort keys such as `updated_at`.
- `thread/start` plus `turn/start` created a thread, and that thread appeared in the current Codex app workspace with the AgentWatcher repository as cwd.
- `codex://threads/<threadId>` opens an existing thread through the Windows protocol handler.
- `codex.cmd app <AgentWatcher repo>` exits successfully and opens or focuses the workspace.
- `codex app-server daemon` is not supported on Windows, so do not depend on daemon/proxy sockets.

## Implementation Changes

- Add `provider: "codex"` sessions with `providerLabel: "CX"`.
- Add `includeCodex` scan setting, defaulting on.
- Scan Codex through a local app-server WebSocket client:
  - start `codex app-server --listen ws://127.0.0.1:<free-port>`;
  - wait for `/readyz`;
  - call `initialize`, then `thread/list`, then `thread/turns/list` for recent turns;
  - map Codex status to AgentWatcher status: `active + waitingOnApproval/waitingOnUserInput` -> `waiting`, plain `active` -> `running`, recent `notLoaded` -> `running`, old `notLoaded/idle/systemError` -> `idle`.
- Create Codex handoff and AgentTask dispatch with `thread/start` plus `turn/start`, passing the target workspace as `cwd` and the prepared prompt as a text input item.
- Open Codex sessions with `codex://threads/<threadId>`; fall back to `codex.cmd app <workspace>`.
- Keep VS Code Bridge untouched; Codex does not use it.

## Test Plan

- Rust tests for Codex status mapping, latest turn extraction, live handoff/task session creation, and `ScanOptions.includeCodex` defaulting on.
- UI tests/manual checks for English/Chinese and dark/light provider labels, settings, handoff mode, and AgentTask dispatch mode.
- Run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `node --check <VS Code 连接组件入口>`
  - `node .tmp\bridge-handoff-routing-test.cjs`
  - `npm run build:ui`
  - `git diff --check`

## Assumptions

- Scope is local Codex Desktop threads in the Windows Codex home/workspace, not Codex Cloud, Slack, Linear, or remote hosts.
- `codex://threads/new` remains useful as a manual fallback only; it is not the main dispatch transport.
- App-server WebSocket is experimental, so the integration should fail soft and keep Copilot/Claude scanning working when Codex is unavailable.
