# AgentWatcher Bridge

Small VS Code extension used by AgentWatcher to reveal or create a specific chat session from a `sessionResource` URI.

Extension ID:

```text
agentwatcher.agentwatcher-vscode-session-bridge-safe4
```

URI format:

```text
vscode://agentwatcher.agentwatcher-vscode-session-bridge-safe4/open?resource=<encoded-session-resource>&target=editor
```

Targets:

- `editor`: open or reveal the session in the current editor group.
- `side`: open to the side.
- `window`: open in a compact chat window.

## Install / Update

AgentWatcher installs or updates this extension automatically when the desktop app starts and finds a missing or older Bridge.

Manual install from a release folder:

```powershell
code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

Manual install from the repository root:

```powershell
code --install-extension .\artifacts\AgentWatcher\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

## Reinstall

```powershell
code --uninstall-extension agentwatcher.agentwatcher-vscode-session-bridge-safe4
code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

Older `safe1`, `safe2`, and `safe3` builds were temporary test authorities. They are not used by the current release path.

## Troubleshooting

- `code --version` must work from PowerShell or AgentWatcher needs to find VS Code in its standard install locations.
- `code --list-extensions | findstr agentwatcher` should show `agentwatcher.agentwatcher-vscode-session-bridge-safe4` after install.
- If URI routing does not work after install, restart VS Code once so the `onUri` activation path is registered.
- If session jump opens only the workspace, confirm the session JSONL path still exists and the Bridge extension is enabled.
- AgentWatcher handoff acknowledgement files are only accepted under the system temp `AgentWatcher` directory; files under project directories are intentionally ignored.