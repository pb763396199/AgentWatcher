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
const defaultProfileDir = path.join(tmpRoot, 'tauri-dev-webview2');
const preferredPort = Number(process.env.AGENTWATCHER_DEV_CDP_PORT || 9222);

const cdpPort = await findFreePort(preferredPort);
const webviewProfileDir = process.env.WEBVIEW2_USER_DATA_FOLDER || defaultProfileDir;
fs.mkdirSync(webviewProfileDir, { recursive: true });
fs.mkdirSync(tmpRoot, { recursive: true });

const env = {
  ...process.env,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: ensureRemoteDebuggingPort(
    process.env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS || '',
    cdpPort,
  ),
  WEBVIEW2_USER_DATA_FOLDER: webviewProfileDir,
  AGENTWATCHER_DEV_CDP_PORT: String(cdpPort),
};

const runtime = {
  mode: 'tauri-dev-cdp',
  cdpPort,
  cdpUrl: `http://127.0.0.1:${cdpPort}`,
  devUrl: 'http://127.0.0.1:1420',
  webviewProfileDir,
  startedAt: new Date().toISOString(),
  command: 'tauri dev',
};
fs.writeFileSync(runtimePath, JSON.stringify(runtime, null, 2), 'utf8');

console.log(`[AgentWatcher dev] WebView2 CDP: ${runtime.cdpUrl}`);
console.log(`[AgentWatcher dev] Runtime: ${runtimePath}`);

const command = process.platform === 'win32' ? (process.env.ComSpec || 'cmd.exe') : 'npx';
const args = process.platform === 'win32'
  ? ['/d', '/s', '/c', 'tauri dev']
  : ['tauri', 'dev'];

const child = spawn(command, args, {
  cwd: repoRoot,
  env,
  stdio: 'inherit',
  windowsHide: true,
});

child.on('exit', code => {
  process.exit(code ?? 0);
});

child.on('error', error => {
  console.error(`[AgentWatcher dev] 启动 tauri dev 失败：${error.message}`);
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
