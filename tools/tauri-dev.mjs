#!/usr/bin/env node

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');
const tmpRoot = path.join(repoRoot, '.tmp');
const runtimePath = path.join(tmpRoot, 'tauri-dev-runtime.json');
const processStatePath = path.join(tmpRoot, 'tauri-dev-process.json');
const stdoutLogPath = path.join(tmpRoot, 'tauri-dev-stdout.log');
const stderrLogPath = path.join(tmpRoot, 'tauri-dev-stderr.log');
const defaultProfileDir = path.join(tmpRoot, 'tauri-dev-webview2');
const preferredPort = Number(process.env.AGENTWATCHER_DEV_CDP_PORT || 9222);
const restart = process.argv.includes('--restart');
const webviewProfileDir = process.env.WEBVIEW2_USER_DATA_FOLDER || defaultProfileDir;
const silent = process.env.AGENTWATCHER_DEV_SILENT === '1';
const debugSilent = process.env.AGENTWATCHER_DEBUG_SILENT !== '0';

fs.mkdirSync(tmpRoot, { recursive: true });

if (restart) {
  await stopPreviousDevRun(processStatePath);
}

const cdpPort = await findFreePort(preferredPort);
fs.mkdirSync(webviewProfileDir, { recursive: true });

const env = {
  ...process.env,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: ensureRemoteDebuggingPort(
    process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS || '',
    cdpPort,
  ),
  WEBVIEW2_USER_DATA_FOLDER: webviewProfileDir,
  AGENTWATCHER_DEV_CDP_PORT: String(cdpPort),
  AGENTWATCHER_DEBUG_SILENT: debugSilent ? '1' : '0',
};

const runtime = {
  mode: 'tauri-dev-cdp',
  cdpPort,
  cdpUrl: `http://127.0.0.1:${cdpPort}`,
  devUrl: 'http://127.0.0.1:1420',
  webviewProfileDir,
  debugSilent,
  startedAt: new Date().toISOString(),
  command: 'tauri dev',
  launcherPid: process.pid,
  stdoutLog: stdoutLogPath,
  stderrLog: stderrLogPath,
};
fs.writeFileSync(runtimePath, JSON.stringify(runtime, null, 2), 'utf8');

console.log(`[AgentWatcher dev] WebView2 CDP: ${runtime.cdpUrl}`);
console.log(`[AgentWatcher dev] Runtime: ${runtimePath}`);

const command = process.platform === 'win32' ? (process.env.ComSpec || 'cmd.exe') : 'npx';
const args = process.platform === 'win32'
  ? ['/d', '/s', '/c', 'npx tauri dev']
  : ['tauri', 'dev'];
const stdio = silent
  ? [
      'ignore',
      fs.openSync(stdoutLogPath, 'a'),
      fs.openSync(stderrLogPath, 'a'),
    ]
  : 'inherit';

const child = spawn(command, args, {
  cwd: repoRoot,
  env,
  stdio,
  windowsHide: silent,
});

fs.writeFileSync(processStatePath, JSON.stringify({
  startedAt: runtime.startedAt,
  repoRoot,
  launcherPid: process.pid,
  childPid: child.pid,
  runtimePath,
}, null, 2), 'utf8');

child.on('exit', code => {
  writeExitState(code ?? 0);
  process.exit(code ?? 0);
});

child.on('error', error => {
  console.error(`[AgentWatcher dev] 启动 tauri dev 失败：${error.message}`);
  writeExitState(null, error.message);
  process.exit(1);
});

function ensureRemoteDebuggingPort(existingArgs, port) {
  const trimmed = existingArgs.trim();
  if (/(^|\s)--remote-debugging-port(=|\s)/.test(trimmed)) return trimmed;
  return `${trimmed} --remote-debugging-port=${port}`.trim();
}

async function findFreePort(startPort) {
  for (let port = startPort; port < startPort + 80; port += 1) {
    if (await canListen(port)) return port;
  }
  throw new Error(`没有找到可用 WebView2 CDP 端口，起始端口：${startPort}`);
}

function canListen(port) {
  return new Promise(resolve => {
    const server = net.createServer();
    server.once('error', () => resolve(false));
    server.once('listening', () => {
      server.close(() => resolve(true));
    });
    server.listen(port, '127.0.0.1');
  });
}

async function stopPreviousDevRun(statePath) {
  if (!fs.existsSync(statePath)) return;

  let state;
  try {
    state = JSON.parse(fs.readFileSync(statePath, 'utf8'));
  } catch {
    return;
  }

  const pids = [state.childPid, state.launcherPid]
    .filter(pid => Number.isInteger(pid) && pid > 0 && pid !== process.pid);

  for (const pid of pids) {
    await taskkill(pid);
  }
}

function taskkill(pid) {
  return new Promise(resolve => {
    if (process.platform !== 'win32') {
      try {
        process.kill(pid, 'SIGTERM');
      } catch {
        // Already gone.
      }
      resolve();
      return;
    }

    const killer = spawn('taskkill.exe', ['/PID', String(pid), '/T', '/F'], {
      stdio: 'ignore',
      windowsHide: true,
    });
    killer.on('exit', () => resolve());
    killer.on('error', () => resolve());
  });
}

function writeExitState(code, error = null) {
  try {
    fs.writeFileSync(processStatePath, JSON.stringify({
      repoRoot,
      launcherPid: process.pid,
      childPid: child.pid,
      runtimePath,
      exitedAt: new Date().toISOString(),
      exitCode: code,
      error,
    }, null, 2), 'utf8');
  } catch {
    // Exit diagnostics must not mask the original process result.
  }
}
