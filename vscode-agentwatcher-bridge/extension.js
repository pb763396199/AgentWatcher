const vscode = require('vscode');

const DEFAULT_TARGET = 'editor';
const COMMANDS_BY_TARGET = {
  editor: 'workbench.action.chat.openSessionInEditorGroup',
  side: 'workbench.action.chat.openSessionInNewEditorGroup',
  window: 'workbench.action.chat.openSessionInNewWindow'
};

function activate(context) {
  const output = vscode.window.createOutputChannel('AgentWatcher Bridge');

  async function openSession(resourceText, target = DEFAULT_TARGET) {
    if (!resourceText || typeof resourceText !== 'string') {
      throw new Error('Missing AgentWatcher session resource.');
    }

    const resource = vscode.Uri.parse(resourceText, true);
    const command = COMMANDS_BY_TARGET[target] || COMMANDS_BY_TARGET[DEFAULT_TARGET];
    output.appendLine(`Opening ${resource.toString()} with ${command}`);
    await vscode.commands.executeCommand(command, { resource });
    output.appendLine(`Opened ${resource.toString()}`);
  }

  context.subscriptions.push(
    vscode.window.registerUriHandler({
      async handleUri(uri) {
        try {
          const params = new URLSearchParams(uri.query);
          const resourceText = params.get('resource') || params.get('session');
          const target = params.get('target') || DEFAULT_TARGET;
          await openSession(resourceText, target);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          output.appendLine(`Failed: ${message}`);
          void vscode.window.showErrorMessage(`AgentWatcher failed to open session: ${message}`);
        }
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('agentwatcher.openSession', async (resourceText, target) => {
      await openSession(resourceText, target || DEFAULT_TARGET);
    })
  );
}

function deactivate() {}

module.exports = { activate, deactivate };