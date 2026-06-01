const vscode = require('vscode');
const fs = require('fs');
const os = require('os');
const path = require('path');

const BRIDGE_LOG_VERSION = '0.1.10-session-bridge';
const DEFAULT_TARGET = 'editor';
const COMMANDS_BY_TARGET = {
  editor: 'workbench.action.chat.openSessionInEditorGroup',
  side: 'workbench.action.chat.openSessionInNewEditorGroup',
  window: 'workbench.action.chat.openSessionInNewWindow'
};
const ALLOWED_COMMANDS = new Set([
  'workbench.action.openChat',
  'workbench.action.chat.open',
  'workbench.action.chat.openInEditor',
  'workbench.action.chat.openNewSessionEditor.local',
  'workbench.action.chat.openNewSessionEditor.copilotcli',
  'workbench.action.chat.openSessionWithPrompt.copilotcli',
  'workbench.action.chat.newChat',
  'github.copilot.cli.newSession',
  'github.copilot.cli.newSessionToSide',
  'github.copilot.cli.openInCopilotCLI',
  'claude-vscode.newConversation',
  'claude-vscode.primaryEditor.open',
  'claude-vscode.editor.open',
  'claude-vscode.editor.openLast',
  'claude-vscode.sidebar.open',
  'claude-vscode.focus'
]);
const COPILOT_HANDOFF_TARGETS = new Map([
  ['workbench.action.chat.openNewSessionEditor.local', { label: 'Copilot editor', command: 'workbench.action.openChat', inputCommand: 'workbench.action.chat.newChat' }],
  ['workbench.action.chat.openInEditor', { label: 'Copilot editor', command: 'workbench.action.openChat', inputCommand: 'workbench.action.chat.newChat' }],
  ['workbench.action.chat.openNewSessionEditor.copilotcli', { label: 'Copilot CLI editor', command: 'workbench.action.chat.openNewSessionEditor.copilotcli', inputCommand: 'workbench.action.chat.newChat', readyDelayMs: 600 }],
  ['github.copilot.cli.newSession', { label: 'Copilot editor', command: 'workbench.action.chat.openNewSessionEditor.local' }],
  ['github.copilot.cli.newSessionToSide', { label: 'Copilot CLI editor', command: 'workbench.action.chat.openNewSessionEditor.copilotcli' }]
]);

function activate(context) {
  const output = vscode.window.createOutputChannel('AgentWatcher Session Bridge');
  output.appendLine(`AgentWatcher Bridge ${BRIDGE_LOG_VERSION} activated`);

  function writeHandoffAck(params, ok, message) {
    const ackPath = params?.get('ackPath');
    const token = params?.get('ackToken');
    if (!ackPath || !token) return;

    const root = path.resolve(os.tmpdir(), 'AgentWatcher') + path.sep;
    const resolved = path.resolve(ackPath);
    if (!resolved.startsWith(root)) {
      output.appendLine(`Skipped handoff acknowledgement outside AgentWatcher temp root: ${resolved}`);
      return;
    }

    fs.mkdirSync(path.dirname(resolved), { recursive: true });
    fs.writeFileSync(resolved, JSON.stringify({ ok, token, message, at: new Date().toISOString() }), 'utf8');
    output.appendLine(`Wrote handoff acknowledgement ok=${ok}`);
  }

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

  async function runAllowedCommand(command) {
    if (!command || !ALLOWED_COMMANDS.has(command)) {
      throw new Error('Unsupported AgentWatcher command.');
    }

    output.appendLine(`Running ${command}`);
    await vscode.commands.executeCommand(command);
    output.appendLine(`Ran ${command}`);
  }

  async function openCopilotHandoffTarget(target, promptText) {
    if (!promptText) {
      output.appendLine(`Opening ${target.label} target with ${target.command} without prompt`);
      await vscode.commands.executeCommand(target.command);
      output.appendLine(`Opened ${target.label} target without prompt`);
      return;
    }

    if (target.inputCommand) {
      output.appendLine(`Opening ${target.label} target with ${target.command} before target-owned input fill (${promptText.length} chars)`);
      await vscode.commands.executeCommand(target.command);
      if (target.readyDelayMs) {
        await new Promise(resolve => setTimeout(resolve, target.readyDelayMs));
      }
      await vscode.commands.executeCommand(target.inputCommand, {
        inputValue: promptText,
        isPartialQuery: true
      });
      output.appendLine(`Filled ${target.label} target through VS Code newChat inputValue path`);
      return;
    }

    output.appendLine(`Opening ${target.label} target with ${target.command} and target-owned initial prompt (${promptText.length} chars)`);
    await vscode.commands.executeCommand(target.command, { prompt: promptText });
    output.appendLine(`Opened ${target.label} target through VS Code target-owned prompt path`);
  }

  async function runHandoffCommand(command, insertPrompt) {
    output.appendLine(`Handoff route command=${command || '(missing)'} insertPrompt=${insertPrompt}`);

    if (insertPrompt && command === 'claude-vscode.editor.open') {
      const text = await vscode.env.clipboard.readText();
      output.appendLine(`Running ${command} with initial prompt (${text ? text.length : 0} chars)`);
      await vscode.commands.executeCommand(command, undefined, text || undefined);
      output.appendLine(`Ran ${command} with initial prompt`);
      return;
    }

    if (insertPrompt && COPILOT_HANDOFF_TARGETS.has(command)) {
      const text = await vscode.env.clipboard.readText();
      const target = COPILOT_HANDOFF_TARGETS.get(command);
      try {
        await openCopilotHandoffTarget(target, text);
        return;
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        output.appendLine(`${target.label} target-bound handoff failed; unsafe focus-based insert was not attempted: ${message}`);
        throw error;
      }
    }

    await runAllowedCommand(command);
    if (insertPrompt) {
      output.appendLine(`Skipped prompt insert for ${command}: no target-bound inserter is available`);
    }
  }

  context.subscriptions.push(
    vscode.window.registerUriHandler({
      async handleUri(uri) {
        let params;
        try {
          params = new URLSearchParams(uri.query);
          const route = (uri.path || '').replace(/^\/+/, '') || 'open';
          if (route === 'handoff') {
            await runHandoffCommand(params.get('command'), params.get('insertPrompt') !== '0');
            writeHandoffAck(params, true, 'Handoff prompt inserted.');
            return;
          }

          if (route === 'command' || params.has('command')) {
            await runAllowedCommand(params.get('command'));
            return;
          }

          const resourceText = params.get('resource') || params.get('session');
          const target = params.get('target') || DEFAULT_TARGET;
          await openSession(resourceText, target);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          output.appendLine(`Failed: ${message}`);
          try {
            writeHandoffAck(params, false, message);
          } catch (ackError) {
            const ackMessage = ackError instanceof Error ? ackError.message : String(ackError);
            output.appendLine(`Failed to write handoff acknowledgement: ${ackMessage}`);
          }
          void vscode.window.showErrorMessage(`AgentWatcher failed to open session: ${message}`);
        }
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand('agentwatcherSessionBridge.openSession', async (resourceText, target) => {
      await openSession(resourceText, target || DEFAULT_TARGET);
    }),
    vscode.commands.registerCommand('agentwatcherSessionBridge.runCommand', async (command) => {
      await runAllowedCommand(command);
    }),
    vscode.commands.registerCommand('agentwatcherSessionBridge.runHandoffCommand', async (command, insertPrompt) => {
      await runHandoffCommand(command, Boolean(insertPrompt));
    })
  );
}

function deactivate() {}

module.exports = { activate, deactivate };