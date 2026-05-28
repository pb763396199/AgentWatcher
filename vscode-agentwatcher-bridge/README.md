# AgentWatcher Bridge

Small VS Code extension used by AgentWatcher to reveal or create a specific chat session from a `sessionResource` URI.

URI format:

```text
vscode://agentwatcher.agentwatcher-bridge/open?resource=<encoded-session-resource>&target=editor
```

Targets:

- `editor`: open or reveal the session in the current editor group.
- `side`: open to the side.
- `window`: open in a compact chat window.