# AgentWatcher Bridge

Small VS Code extension used by AgentWatcher to reveal or create a specific chat session from a `sessionResource` URI.

Current Bridge version: `0.1.12`. The Bridge version is independent from the AgentWatcher app version.

Extension ID:

```text
agentwatcher.agentwatcher-vscode-session-bridge
```

This is the only supported release ID. Historical `safe1`, `safe2`, `safe3`, and `safe4` IDs were temporary test authorities and should not remain installed.

URI format:

```text
vscode://agentwatcher.agentwatcher-vscode-session-bridge/open?resource=<encoded-session-resource>&target=editor
```

Targets:

- `editor`: open or reveal the session in the current editor group.
- `side`: open to the side.
- `window`: open in a compact chat window.

## Install / Update

AgentWatcher installs or updates this extension automatically when the desktop app starts and finds a missing or older Bridge. Install/update also attempts to remove legacy `safe1`-`safe4` IDs before installing the stable VSIX.

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
code --uninstall-extension agentwatcher.agentwatcher-vscode-session-bridge
code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

The stable extension ID is the only supported release identity. Older `safe1`, `safe2`, `safe3`, and `safe4` builds were temporary test authorities; AgentWatcher install/update attempts to remove them before installing the stable VSIX.

## Troubleshooting

- `code --version` must work from PowerShell or AgentWatcher needs to find VS Code in its standard install locations.
- `code --list-extensions | findstr agentwatcher` should show only `agentwatcher.agentwatcher-vscode-session-bridge` after install. Legacy `safe1`-`safe4` IDs should not remain.
- If URI routing does not work after install, restart VS Code once so the `onUri` activation path is registered.
- If session jump opens only the workspace, confirm the session JSONL path still exists and the Bridge extension is enabled.
- AgentWatcher handoff acknowledgement files are only accepted under the system temp `AgentWatcher` directory; files under project directories are intentionally ignored.
