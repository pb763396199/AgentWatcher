#!/usr/bin/env node

import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..', '..');
const tauriConfigPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
const debugExePath = path.join(repoRoot, 'src-tauri', 'target', 'debug', 'agentwatcher.exe');
const tmpRoot = path.join(repoRoot, '.tmp', 'tauri-realtest');
const devRuntimePath = path.join(repoRoot, '.tmp', 'tauri-dev-runtime.json');
const startedProcesses = [];

const runStartedAt = new Date();
const runId = [
  runStartedAt.getFullYear(),
  String(runStartedAt.getMonth() + 1).padStart(2, '0'),
  String(runStartedAt.getDate()).padStart(2, '0'),
  '-',
  String(runStartedAt.getHours()).padStart(2, '0'),
  String(runStartedAt.getMinutes()).padStart(2, '0'),
  String(runStartedAt.getSeconds()).padStart(2, '0'),
].join('');
const outputDir = path.join(tmpRoot, runId);
fs.mkdirSync(outputDir, { recursive: true });

const aliasToFlow = new Map([
  ['smoke', 'smoke'],
  ['冒烟', 'smoke'],
  ['all', 'smoke'],
  ['全部', 'smoke'],
  ['agent-task', 'agent-task'],
  ['agenttask', 'agent-task'],
  ['任务面板', 'agent-task'],
  ['任务', 'agent-task'],
  ['plugin-lifecycle', 'plugin-lifecycle'],
  ['plugins', 'plugin-lifecycle'],
  ['插件生命周期', 'plugin-lifecycle'],
  ['performance', 'performance'],
  ['perf', 'performance'],
  ['性能面板', 'performance'],
  ['性能', 'performance'],
  ['handoff', 'handoff'],
  ['接续面板', 'handoff'],
  ['接续', 'handoff'],
  ['opencode-session', 'opencode-session'],
  ['opencode-card', 'opencode-session'],
  ['opencode', 'opencode-session'],
  ['opencode会话', 'opencode-session'],
  ['opencode卡片', 'opencode-session'],
  ['opencode-handoff-context', 'opencode-handoff-context'],
  ['opencode-handoff', 'opencode-handoff-context'],
  ['opencode接续', 'opencode-handoff-context'],
  ['settings-performance', 'settings-performance'],
  ['settings-perf', 'settings-performance'],
  ['设置性能', 'settings-performance'],
  ['设置卡顿', 'settings-performance'],
  ['filtering', 'filtering'],
  ['filters', 'filtering'],
  ['筛选', 'filtering'],
  ['主面板筛选', 'filtering'],
]);

const flowDisplay = {
  smoke: '冒烟流程',
  'agent-task': 'AgentTask 面板流程',
  'plugin-lifecycle': '插件三态生命周期流程',
  performance: '性能面板流程',
  handoff: '接续面板流程',
  'opencode-session': 'OpenCode 会话卡片流程',
  'opencode-handoff-context': 'OpenCode 接续上下文流程',
  'settings-performance': '设置切换性能流程',
  filtering: '主面板筛选流程',
};

const args = parseArgs(process.argv.slice(2));
const config = readJson(tauriConfigPath);
const devUrl = config?.build?.devUrl || 'http://127.0.0.1:1420';
const timeoutMs = Number(args.options.timeout || 180000);
const hardTimeoutMs = Number(args.options.hardTimeout || args.options['hard-timeout'] || (timeoutMs + 30000));
const requestedPort = Number(args.options.port || 9222);
const attachOnly = Boolean(args.options.attach);
const isolatedRuntime = Boolean(args.options.isolated || args.options['isolated-runtime']);
const keepOpen = Boolean(args.options.keepOpen || args.options['keep-open']);
const focusDuringTest = args.options.focus === true
  || args.options.focus === '1'
  || args.options.focus === 'true'
  || args.options.focus === 'yes';
const silentDebug = !focusDuringTest;
const selectedFlows = normalizeFlows(args.flows);

const result = {
  工具: 'AgentWatcher Tauri 真实交互测试',
  说明: '通过 Playwright 连接 Windows WebView2 CDP，验证真实 Tauri 桌面壳，不使用浏览器 mock。',
  开始时间: runStartedAt.toISOString(),
  仓库: repoRoot,
  开发地址: devUrl,
  输出目录: outputDir,
  静默调试: silentDebug,
  流程: selectedFlows.map(flow => flowDisplay[flow] || flow),
  步骤: [],
  窗口: [],
  截图: [],
  诊断: {
    浏览器日志: [],
    页面错误: [],
  },
  通过: false,
};

let browser;
let context;
let cdpPort;
let launchMode = '未知';

process.on('SIGINT', () => shutdown(130));
process.on('SIGTERM', () => shutdown(143));
const hardTimeoutTimer = setTimeout(() => {
  result.通过 = false;
  result.错误 = `测试超过硬超时：${hardTimeoutMs}ms`;
  result.结束时间 = new Date().toISOString();
  writeResult();
  cleanupStartedProcesses();
  process.exit(124);
}, hardTimeoutMs);

try {
  log('启动真实 Tauri 交互测试');

  if (attachOnly) {
    cdpPort = requestedPort;
    result.CDP端口 = cdpPort;
    log(`使用已存在的 CDP 端口：${cdpPort}`);
    await waitForCdp(cdpPort, 5000);
    launchMode = '附着已有运行时';
  } else {
    const devRuntime = isolatedRuntime ? null : await readReadyDevRuntime(5000);
    if (devRuntime) {
      cdpPort = devRuntime.cdpPort;
      result.CDP端口 = cdpPort;
      result.Dev运行时 = devRuntime;
      launchMode = '附着 npm run dev 的 CDP 运行时';
      log(`附着 npm run dev 的 CDP 端口：${cdpPort}`);
    } else {
      cdpPort = await findFreePort(requestedPort);
      result.CDP端口 = cdpPort;
      launchMode = await launchRuntime(cdpPort);
      await waitForCdp(cdpPort, timeoutMs);
    }
  }

  result.启动方式 = launchMode;
  result.WebView2 = await fetchJson(`http://127.0.0.1:${cdpPort}/json/version`);

  const playwright = await import('playwright-core');
  browser = await connectOverCdpWithRetry(playwright.chromium, cdpPort, 30000);
  context = browser.contexts()[0];
  attachDiagnostics(context);
  await configureSilentDebug(context);

  await runStep('切换主窗口界面语言为中文', () => ensureChineseUi());

  for (const flow of selectedFlows) {
    if (flow === 'smoke') {
      await runStep('确认主窗口是真实 Tauri 运行时', () => assertMainWindow());
      await runStep('点击打开性能诊断窗口', () => runPerformanceFlow());
      await runStep('确认接续面板 WebViewWindow 可被识别', () => runHandoffFlow());
      await runStep('点击切换到 AgentTask 面板', () => runAgentTaskFlow());
    } else if (flow === 'agent-task') {
      await runStep('点击切换到 AgentTask 面板', () => runAgentTaskFlow());
    } else if (flow === 'plugin-lifecycle') {
      await runStep('验证插件未挂载、已挂载未启用、已启用三种状态', () => runPluginLifecycleFlow());
    } else if (flow === 'performance') {
      await runStep('点击打开性能诊断窗口', () => runPerformanceFlow());
    } else if (flow === 'handoff') {
      await runStep('确认接续面板 WebViewWindow 可被识别', () => runHandoffFlow());
    } else if (flow === 'opencode-session') {
      await runStep('点击真实 OpenCode 会话卡片', () => runOpenCodeSessionFlow());
    } else if (flow === 'opencode-handoff-context') {
      await runStep('验证 OpenCode 接续 Prompt 可读取来源文件', () => runOpenCodeHandoffContextFlow());
    } else if (flow === 'settings-performance') {
      await runStep('打开子面板后测量设置切换延迟', () => runSettingsPerformanceFlow());
    } else if (flow === 'filtering') {
      await runStep('验证主面板 provider、搜索、空状态和 todo workspace 过滤', () => runFilteringFlow());
    }
  }

  await collectWindows();
  result.通过 = result.步骤.every(step => step.通过);
  result.结束时间 = new Date().toISOString();
  writeResult();

  if (result.通过) {
    log(`通过：${selectedFlows.map(flow => flowDisplay[flow]).join('、')}`);
    log(`结果：${path.join(outputDir, 'result.json')}`);
  } else {
    fail('有步骤失败，查看 result.json。');
  }
} catch (error) {
  result.通过 = false;
  result.错误 = errorMessage(error);
  result.结束时间 = new Date().toISOString();
  await collectWindows().catch(() => {});
  await captureFailureScreenshot().catch(() => {});
  writeResult();
  fail(result.错误);
} finally {
  clearTimeout(hardTimeoutTimer);
  if (browser) await browser.close().catch(() => {});
  if (!keepOpen) {
    cleanupStartedProcesses();
  } else {
    detachStartedProcesses();
  }
}

if (!result.通过) process.exit(1);

function parseArgs(rawArgs) {
  const options = {};
  const flows = [];

  for (const arg of rawArgs) {
    if (arg.startsWith('--')) {
      const withoutPrefix = arg.slice(2);
      const equalIndex = withoutPrefix.indexOf('=');
      if (equalIndex >= 0) {
        options[withoutPrefix.slice(0, equalIndex)] = withoutPrefix.slice(equalIndex + 1);
      } else {
        options[withoutPrefix] = true;
      }
    } else {
      flows.push(arg);
    }
  }

  return { options, flows };
}

async function configureSilentDebug(browserContext) {
  if (!silentDebug) return;
  const markSilent = () => {
    window.__agentWatcherSilentDebug = true;
    try {
      localStorage.setItem('agentwatcher.debug.silent', '1');
    } catch {
      // Storage may be unavailable on transient pages; runtime env still covers Tauri.
    }
  };
  await browserContext.addInitScript(markSilent).catch(() => {});
  await Promise.all(browserContext.pages().map(page => page.evaluate(markSilent).catch(() => {})));
}

async function maybeBringToFront(page) {
  if (silentDebug) return;
  await page.bringToFront().catch(() => {});
}

function normalizeFlows(rawFlows) {
  if (rawFlows.length === 0) return ['smoke'];

  const flows = rawFlows.map(flow => {
    const normalized = aliasToFlow.get(String(flow).trim().toLowerCase());
    if (!normalized) {
      throw new Error(`未知流程：${flow}。可用流程：冒烟、任务面板、性能面板、接续面板、opencode-session、opencode-handoff-context、settings-performance、filtering。`);
    }
    return normalized;
  });

  return [...new Set(flows)];
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

async function readReadyDevRuntime(waitMs) {
  if (!fs.existsSync(devRuntimePath)) return null;

  let runtime;
  try {
    runtime = readJson(devRuntimePath);
  } catch {
    return null;
  }

  const port = Number(runtime?.cdpPort);
  if (!Number.isFinite(port) || port <= 0) return null;

  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    try {
      await fetchJson(`http://127.0.0.1:${port}/json/version`);
      return {
        ...runtime,
        cdpPort: port,
        runtimePath: devRuntimePath,
      };
    } catch {
      await delay(250);
    }
  }

  return null;
}

async function launchRuntime(port) {
  const profileDir = path.join(outputDir, 'webview2-profile');
  fs.mkdirSync(profileDir, { recursive: true });

  const env = {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    WEBVIEW2_USER_DATA_FOLDER: profileDir,
    AGENTWATCHER_DEBUG_SILENT: silentDebug ? '1' : '0',
  };

  const devServerReady = await isDevServerReady(devUrl, 5000);
  if (devServerReady && fs.existsSync(debugExePath)) {
    log('检测到 Vite 已在运行，直接启动 debug exe。');
    const app = spawn(debugExePath, [], {
      cwd: path.join(repoRoot, 'src-tauri'),
      env,
      windowsHide: true,
      stdio: ['ignore', 'ignore', 'ignore'],
    });
    startedProcesses.push(app);
    return '复用已有 Vite，启动 debug exe';
  }

  log(devServerReady ? '检测到 Vite 已在运行，准备 debug exe。' : '未检测到可复用的 Vite，启动 Vite。');
  const logPath = path.join(outputDir, 'tauri-dev.log');
  const logStream = fs.createWriteStream(logPath, { flags: 'a' });
  result.开发日志 = logPath;

  if (!devServerReady) {
    const vite = spawnNpmDevUi(env);
    pipeChildToLog(vite, logStream);
    startedProcesses.push(vite);
    if (!(await isDevServerReady(devUrl, 60000))) {
      throw new Error(`等待 Vite dev server 超时：${devUrl}`);
    }
  }

  if (!fs.existsSync(debugExePath)) {
    log('debug exe 不存在，先构建 src-tauri debug 版本。');
    const cargo = spawn('cargo', ['build', '--manifest-path', path.join(repoRoot, 'src-tauri', 'Cargo.toml')], {
      cwd: repoRoot,
      env,
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    pipeChildToLog(cargo, logStream);
    await waitForProcessExit(cargo, 'cargo build src-tauri');
  }

  const app = spawn(debugExePath, [], {
    cwd: path.join(repoRoot, 'src-tauri'),
    env,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  pipeChildToLog(app, logStream);
  startedProcesses.push(app);
  return devServerReady ? '复用已有 Vite，启动 debug exe' : '启动 Vite 和 debug exe';
}

async function assertMainWindow() {
  const page = await getMainPage();
  const info = await assertTauriPage(page, '主窗口');
  const silentRuntime = await page.evaluate(() => ({
    windowFlag: window.__agentWatcherSilentDebug === true,
    storageFlag: localStorage.getItem('agentwatcher.debug.silent') === '1',
  })).catch(() => ({}));
  const screenshot = await saveScreenshot(page, 'main-tauri.png');
  return { ...info, 静默调试契约: assertSilentDebugContract(), 静默调试运行时: silentRuntime, 截图: screenshot };
}

function assertSilentDebugContract() {
  const uiSource = fs.readFileSync(path.join(repoRoot, 'ui', 'index.html'), 'utf8');
  const testSource = fs.readFileSync(fileURLToPath(import.meta.url), 'utf8');
  const helperOnlyBringToFront = (testSource.match(/\.bringToFront\(/g) || []).length === 1
    && testSource.includes('async function maybeBringToFront');
  const checks = {
    测试默认静默: /const silentDebug = !focusDuringTest/.test(testSource),
    测试不直接抢前台: helperOnlyBringToFront,
    UI有静默调试开关: uiSource.includes('function isSilentDebugMode()'),
    原生窗口焦点受控: uiSource.includes('function focusNativeWindowIfAllowed')
      && !/windowRef\.setFocus\(\)\.catch/.test(uiSource.replace(/function focusNativeWindowIfAllowed[\s\S]{0,180}/, '')),
    原生窗口置顶受控: uiSource.includes('function effectiveAlwaysOnTop()')
      && !/alwaysOnTop:\s*!!settings\.alwaysOnTop/.test(uiSource),
    子窗口创建焦点受控: !/focus:\s*true/.test(uiSource)
      && uiSource.includes('focus: nativeWindowShouldFocus'),
  };
  if (!Object.values(checks).every(Boolean)) {
    throw new Error(`静默调试契约失败：${JSON.stringify(checks)}`);
  }
  return checks;
}

async function ensureChineseUi() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');

  const before = await page.evaluate(() => ({
    htmlLang: document.documentElement.lang,
    bodyLang: document.body.dataset.lang || '',
  }));

  if (before.htmlLang === 'zh-CN' || before.bodyLang === 'zh') {
    return { 操作: '已是中文界面', 切换前: before, 切换后: before };
  }

  await page.locator('#settingsToggle').click();
  const zhButton = page.locator('[data-setting-lang="zh"]');
  try {
    await zhButton.waitFor({ state: 'visible', timeout: 3000 });
  } catch {
    return { 操作: '语言切换控件当前不可见，保持当前语言继续目标流程', 切换前: before, 切换后: before };
  }
  await zhButton.click();
  await page.waitForFunction(() => document.documentElement.lang === 'zh-CN' && document.body.dataset.lang === 'zh', null, { timeout: 10000 });
  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => document.documentElement.lang === 'zh-CN' && document.body.dataset.lang === 'zh', null, { timeout: 10000 });
  await page.waitForFunction(() => {
    const workspaceLabel = document.querySelector('#workspacePickerValue')?.textContent || '';
    const statusText = [...document.querySelectorAll('.segment[data-status]')]
      .map(segment => segment.textContent || '')
      .join(' ');
    return workspaceLabel.includes('全部工作区') && statusText.includes('全部');
  }, null, { timeout: 15000 });

  const after = await page.evaluate(() => ({
    htmlLang: document.documentElement.lang,
    bodyLang: document.body.dataset.lang || '',
    工作区筛选: document.querySelector('#workspacePickerValue')?.textContent || '',
    状态筛选: [...document.querySelectorAll('.segment[data-status]')]
      .map(segment => segment.textContent?.trim() || '')
      .join(' / '),
  }));

  return { 操作: '通过设置面板切换到中文界面', 切换前: before, 切换后: after };
}

async function runAgentTaskFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');
  const todoBackup = snapshotTodoStateFile();
  const taskTitle = `Realtest plugin workflow ${runId}`;
  try {
    await openSettingsPanel(page);
    await page.locator('#pluginRefresh').click({ force: true });
    await page.waitForFunction(() => {
      const panel = document.querySelector('#pluginPanel');
      return panel && panel.innerText.includes('插件管理');
    }, null, { timeout: 15000 });
    const pluginPanel = await page.evaluate(() => ({
      标题: document.querySelector('#pluginTitle')?.textContent?.trim() || '',
      内容: document.querySelector('#pluginPanel')?.innerText || '',
      使用通用插件控件: !!document.querySelector('#pluginMount, #pluginRefresh, #pluginList'),
    }));
    if (pluginPanel.标题 !== '插件管理' || !pluginPanel.使用通用插件控件) {
      throw new Error(`插件管理仍依赖旧专用界面：${JSON.stringify(pluginPanel)}`);
    }
    await page.locator('#settingsToggle').click({ force: true }).catch(() => {});
    const workspace = await page.evaluate(() => {
      const card = document.querySelector('.lane .session-card[data-workspace-key]');
      return card ? {
        key: card.dataset.workspaceKey || '',
        path: card.dataset.workspacePath || '',
        label: card.dataset.workspaceLabel || card.dataset.workspace || 'Realtest Workspace'
      } : null;
    });
    if (!workspace?.key) throw new Error('没有可用于创建测试任务的会话工作区。');
    const state = todoBackup.existed ? safeJsonParse(todoBackup.text, { version: 1, workspaces: {} }) : { version: 1, workspaces: {} };
    state.version = 1;
    state.workspaces = state.workspaces && typeof state.workspaces === 'object' ? state.workspaces : {};
    const workspaceState = state.workspaces[workspace.key] || { key: workspace.key, label: workspace.label, path: workspace.path, tasks: [] };
    workspaceState.key = workspace.key;
    workspaceState.label = workspace.label;
    workspaceState.path = workspace.path;
    workspaceState.tasks = Array.isArray(workspaceState.tasks) ? workspaceState.tasks : [];
    workspaceState.tasks.unshift({
      id: `realtest-plugin-workflow-${runId}`,
      title: taskTitle,
      description: '验证外部插件贡献的动态任务表单。',
      acceptance: '工作流和目标 Agent 分开，字段来自插件 schema。',
      workflow: 'standard',
      launchMode: 'agents',
      status: 'ready',
      order: 0,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      dispatches: []
    });
    workspaceState.tasks.forEach((task, index) => { task.order = index; });
    state.workspaces[workspace.key] = workspaceState;
    fs.mkdirSync(path.dirname(todoBackup.path), { recursive: true });
    fs.writeFileSync(todoBackup.path, JSON.stringify(state, null, 2), 'utf8');
    await page.reload({ waitUntil: 'domcontentloaded' });
    const inTaskMode = await page.locator('body').evaluate(body => body.classList.contains('is-agent-task-mode'));
    if (!inTaskMode) await page.locator('#todoToggle').click({ force: true });
    await page.waitForFunction(() => document.body.classList.contains('is-agent-task-mode'), null, { timeout: 10000 });
    await page.locator('#agentTaskWorkspaceButton').click({ force: true });
    const workspaceOption = page.locator(`#agentTaskWorkspaceOptions .handoff-picker-option[data-workspace-key="${workspace.key}"]`).first();
    if (!(await workspaceOption.count())) throw new Error(`任务工作区选项不存在：${workspace.key}`);
    await workspaceOption.click({ force: true });
    await page.waitForFunction(title => [...document.querySelectorAll('.agenttask-item')].some(card => card.innerText.includes(title)), taskTitle, { timeout: 10000 });
    const card = page.locator('.agenttask-item').filter({ hasText: taskTitle }).first();
    await card.locator('.agenttask-item-main').click({ force: true });
    await page.locator('#agentTaskWorkflowButton').click({ force: true });
    const workflowOption = page.locator('#agentTaskWorkflowOptions [data-mode^="plugin:"]').first();
    const workflowCount = await workflowOption.count();
    if (!workflowCount) {
      const zeroPluginSurface = await page.evaluate(() => ({
        工作流值: document.querySelector('#agentTaskWorkflowSelect')?.value || '',
        目标Agent值: document.querySelector('#agentTaskModeSelect')?.value || '',
        插件工作流数量: [...document.querySelectorAll('#agentTaskWorkflowOptions [data-mode^="plugin:"]')].length,
        动态表单隐藏: !!document.querySelector('#agentTaskPluginWorkflowContext')?.hidden,
        工作流与Agent分开: !!document.querySelector('#agentTaskWorkflowButton') && !!document.querySelector('#agentTaskModeButton'),
      }));
      if (zeroPluginSurface.插件工作流数量 !== 0
        || !zeroPluginSurface.动态表单隐藏
        || !zeroPluginSurface.目标Agent值
        || !zeroPluginSurface.工作流与Agent分开
        || !pluginPanel.内容.includes('零插件模式')) {
        throw new Error(`零插件 AgentTask 安全空态验收失败：${JSON.stringify({ pluginPanel, zeroPluginSurface })}`);
      }
      await page.keyboard.press('Escape').catch(() => {});
      const screenshot = await saveViewportScreenshot(page, 'zero-plugin-agenttask.png');
      return { 插件管理: pluginPanel, 零插件任务面板: zeroPluginSurface, 截图: [screenshot] };
    }
    const workflowId = await workflowOption.getAttribute('data-mode');
    await page.evaluate(id => {
      const select = document.querySelector('#agentTaskWorkflowSelect');
      select.value = id;
      select.dispatchEvent(new Event('change', { bubbles: true }));
    }, workflowId);
    await page.waitForFunction(() => {
      const form = document.querySelector('#agentTaskPluginWorkflowContext');
      return form && !form.hidden && form.querySelectorAll('[data-plugin-workflow-key]').length > 0;
    }, null, { timeout: 10000 });
    const taskSurface = await page.evaluate(() => ({
      工作流值: document.querySelector('#agentTaskWorkflowSelect')?.value || '',
      目标Agent值: document.querySelector('#agentTaskModeSelect')?.value || '',
      动态字段: [...document.querySelectorAll('[data-plugin-workflow-key]')].map(field => field.dataset.pluginWorkflowKey),
      只有Schema字段: [...document.querySelectorAll('#agentTaskPluginWorkflowFields input, #agentTaskPluginWorkflowFields textarea, #agentTaskPluginWorkflowFields select')]
        .every(field => !!field.dataset.pluginWorkflowKey),
      工作流与Agent分开: !!document.querySelector('#agentTaskWorkflowButton') && !!document.querySelector('#agentTaskModeButton'),
    }));
    if (taskSurface.工作流值 !== workflowId || !taskSurface.目标Agent值 || !taskSurface.动态字段.length || !taskSurface.只有Schema字段 || !taskSurface.工作流与Agent分开) {
      throw new Error(`插件工作流动态表单验收失败：${JSON.stringify(taskSurface)}`);
    }
    await page.waitForFunction(() => {
      const face = document.querySelector('#agentTaskFace');
      const body = document.body;
      return body.classList.contains('is-agent-task-mode')
        && !body.classList.contains('is-agent-task-flipping')
        && face?.getAttribute('aria-hidden') === 'false';
    }, null, { timeout: 5000 });
    await page.locator('#agentTaskPluginWorkflowContext').scrollIntoViewIfNeeded();
    const screenshot = await saveViewportScreenshot(page, 'plugin-workflow-task-schema.png');
    return { 插件管理: pluginPanel, 任务创建: taskSurface, 截图: [screenshot] };
  } finally {
    restoreTodoStateFile(todoBackup);
    await page.reload({ waitUntil: 'domcontentloaded' }).catch(() => {});
  }
}

async function runPluginLifecycleFlow() {
  const pluginRoot = process.env.AGENTWATCHER_LIVE_PLUGIN_ROOT;
  if (!pluginRoot) throw new Error('AGENTWATCHER_LIVE_PLUGIN_ROOT is required for plugin lifecycle flow.');
  const descriptorPath = fs.readdirSync(pluginRoot)
    .map(name => path.join(pluginRoot, name))
    .find(candidate => candidate.toLowerCase().endsWith('.awplugin'));
  if (!descriptorPath) throw new Error(`插件目录没有 *.awplugin：${pluginRoot}`);
  const descriptor = readJson(descriptorPath);
  const pluginName = descriptor?.Name;
  if (!pluginName) throw new Error(`插件描述文件缺少 Name：${descriptorPath}`);
  const appData = process.env.APPDATA;
  if (!appData) throw new Error('APPDATA is required for plugin lifecycle flow.');
  const mountFile = path.join(appData, 'AgentWatcher', 'plugin-mounts.v1.json');
  const stateFile = path.join(appData, 'AgentWatcher', 'plugins.v1.json');
  const backups = [mountFile, stateFile].map(file => ({ file, existed: fs.existsSync(file), text: fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '' }));
  const page = await getMainPage();
  const writeState = (mounted, enabled) => {
    fs.mkdirSync(path.dirname(mountFile), { recursive: true });
    fs.writeFileSync(mountFile, JSON.stringify({ version: 1, mounts: mounted ? [pluginRoot] : [] }, null, 2), 'utf8');
    fs.writeFileSync(stateFile, JSON.stringify({ version: 1, plugins: enabled ? { [pluginName]: { enabled: true, updatedMs: Date.now() } } : {}, audit: [] }, null, 2), 'utf8');
  };
  const reloadCatalog = async (expectedText) => {
    await page.reload({ waitUntil: 'domcontentloaded' });
    await assertTauriPage(page, '主窗口');
    await openSettingsPanel(page);
    await page.locator('#pluginRefresh').click({ force: true }).catch(() => {});
    if (expectedText) {
      await page.waitForFunction(text => (document.querySelector('#pluginPanel')?.innerText || '').includes(text), expectedText, { timeout: 15000 });
    }
  };
  try {
    writeState(false, false);
    await reloadCatalog('零插件模式');
    const zero = await page.evaluate(() => ({
      panel: document.querySelector('#pluginPanel')?.innerText || '',
      workflows: [...document.querySelectorAll('#agentTaskWorkflowSelect option')].map(option => option.value)
    }));
    if (!zero.panel.includes('零插件模式') || zero.workflows.some(value => value.startsWith('plugin:'))) {
      throw new Error(`零插件状态不正确：${JSON.stringify(zero)}`);
    }

    writeState(true, false);
    await reloadCatalog(descriptor.FriendlyName || pluginName);
    const mounted = await page.evaluate(() => ({
      panel: document.querySelector('#pluginPanel')?.innerText || '',
      workflows: [...document.querySelectorAll('#agentTaskWorkflowSelect option')].map(option => option.value)
    }));
    if (!mounted.panel.includes('已挂载，未启用') || mounted.workflows.some(value => value.startsWith('plugin:'))) {
      throw new Error(`挂载未启用状态不正确：${JSON.stringify(mounted)}`);
    }

    writeState(true, true);
    await reloadCatalog(descriptor.FriendlyName || pluginName);
    await page.waitForFunction(() => [...document.querySelectorAll('#agentTaskWorkflowSelect option')].some(option => option.value.startsWith('plugin:')), null, { timeout: 15000 });
    const enabled = await page.evaluate(() => ({
      panel: document.querySelector('#pluginPanel')?.innerText || '',
      workflows: [...document.querySelectorAll('#agentTaskWorkflowSelect option')].map(option => option.value)
    }));
    if (!enabled.panel.includes('已通过启动检查') || !enabled.workflows.some(value => value.startsWith('plugin:'))) {
      throw new Error(`启用状态不正确：${JSON.stringify(enabled)}`);
    }
    return { 未挂载: zero, 已挂载未启用: mounted, 已启用: enabled };
  } finally {
    for (const backup of backups) {
      if (backup.existed) {
        fs.mkdirSync(path.dirname(backup.file), { recursive: true });
        fs.writeFileSync(backup.file, backup.text, 'utf8');
      } else if (fs.existsSync(backup.file)) {
        fs.rmSync(backup.file, { force: true });
      }
    }
    await page.reload({ waitUntil: 'domcontentloaded' }).catch(() => {});
  }
}

async function runPerformanceFlow() {
  const main = await getMainPage();
  await assertTauriPage(main, '主窗口');
  await ensureWatcherSurface(main);
  const toggle = main.locator('#performanceToggle');
  await toggle.waitFor({ state: 'visible', timeout: 10000 });

  const pressedBefore = await toggle.getAttribute('aria-pressed');
  if (pressedBefore === 'true') {
    await toggle.click({ force: true });
    await main.waitForFunction(() => document.querySelector('#performanceToggle')?.getAttribute('aria-pressed') === 'false', null, { timeout: 10000 });
  }

  const pagesBefore = new Set(context.pages());
  await toggle.click({ force: true });

  const surface = await waitForPerformanceSurface(main, 30000);
  const diagnostics = surface.page;
  await diagnostics.waitForLoadState('domcontentloaded').catch(() => {});
  const info = await assertTauriPage(diagnostics, surface.label);
  const text = await visibleText(diagnostics);
  if (!/Performance|性能|diagnostics|诊断/i.test(text)) {
    throw new Error('性能面板没有出现预期文本。');
  }
  const screenshot = await saveScreenshot(diagnostics, 'performance-window.png');

  return {
    ...info,
    打开方式: surface.mode,
    新窗口: !pagesBefore.has(diagnostics),
    点击前ariaPressed: pressedBefore,
    点击后ariaPressed: await toggle.getAttribute('aria-pressed').catch(() => ''),
    可见文本片段: text.slice(0, 180),
    截图: screenshot,
  };
}

async function waitForPerformanceSurface(main, waitMs) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    const page = context.pages().find(candidate => candidate.url().includes('?performance=1'));
    if (page) return { page, mode: '独立 Tauri WebViewWindow', label: '性能诊断窗口' };

    const inlineVisible = await main.locator('#perfPanel').evaluate(element => {
      if (!element || element.hidden) return false;
      const rect = element.getBoundingClientRect();
      const style = window.getComputedStyle(element);
      return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
    }).catch(() => false);
    if (inlineVisible) return { page: main, mode: '主窗口内性能面板', label: '主窗口性能面板' };

    await delay(250);
  }

  const status = await main.evaluate(() => ({
    ariaPressed: document.querySelector('#performanceToggle')?.getAttribute('aria-pressed') || '',
    toast: document.querySelector('#toast')?.textContent || '',
    perfPanelHidden: document.querySelector('#perfPanel')?.hidden ?? null,
    bodyClass: document.body.className,
  })).catch(error => ({ error: String(error) }));
  throw new Error(`等待 性能诊断窗口 超时。当前状态：${JSON.stringify(status)}`);
}

async function runHandoffFlow() {
  const main = await getMainPage();
  await assertTauriPage(main, '主窗口');
  await maybeBringToFront(main);
  await ensureWatcherSurface(main);

  let handoff = context.pages().find(page => page.url().includes('?handoff=1'));
  if (!handoff) {
    await waitForSessionSurface(main);
    await main.waitForFunction(() => {
      return Array.from(document.querySelectorAll('.lane .session-card[data-session-id]:not([hidden])'))
        .some(card => card.dataset.sessionId);
    }, null, { timeout: 30000 });
    const card = main.locator('.lane .session-card[data-session-id]:not([hidden])').first();
    await card.waitFor({ state: 'visible', timeout: 30000 });
    await card.scrollIntoViewIfNeeded();
    await card.click({ button: 'right' });
    await main.locator('#handoffOpen').waitFor({ state: 'visible', timeout: 5000 });
    await main.locator('#handoffOpen').click();
    handoff = await waitForPage(
      page => page.url().includes('?handoff=1'),
      '接续面板窗口',
      12000,
    ).catch(() => null);
    if (!handoff) {
      const state = await main.evaluate(() => {
        const selected = document.querySelector('.lane .session-card[data-session-id]:not([hidden])');
        const openButton = document.querySelector('#handoffOpen');
        const layer = document.querySelector('#handoffLayer');
        return {
          url: location.href,
          selectedCard: selected ? {
            sessionId: selected.dataset.sessionId || '',
            provider: selected.dataset.provider || '',
            text: selected.textContent?.slice(0, 180) || '',
          } : null,
          menuOpen: document.querySelector('#handoffContextMenu')?.classList.contains('open') || false,
          openButtonVisible: !!openButton && getComputedStyle(openButton).display !== 'none' && getComputedStyle(openButton).visibility !== 'hidden',
          openButtonOnClick: typeof openButton?.onclick,
          layerHidden: layer?.hidden ?? null,
          inlineVisible: !!layer && !layer.hidden,
          sourceLabel: document.querySelector('#handoffSourceLabel')?.textContent || '',
          toast: document.querySelector('#toast')?.textContent || '',
        };
      }).catch(e => ({ diagnosticError: String(e) }));
      throw new Error(`Native handoff panel did not open; inline handoff panel fallback is forbidden in Tauri smoke. State: ${JSON.stringify(state)}`);
      try {
        await main.waitForFunction(() => {
          const layer = document.querySelector('#handoffLayer');
          return !!layer && !layer.hidden;
        }, null, { timeout: 10000 });
      } catch (error) {
        const state = await main.evaluate(() => {
          const selected = document.querySelector('.lane .session-card[data-session-id]:not([hidden])');
          const openButton = document.querySelector('#handoffOpen');
          const layer = document.querySelector('#handoffLayer');
          return {
            url: location.href,
            selectedCard: selected ? {
              sessionId: selected.dataset.sessionId || '',
              provider: selected.dataset.provider || '',
              text: selected.textContent?.slice(0, 180) || '',
            } : null,
            menuOpen: document.querySelector('#handoffContextMenu')?.classList.contains('open') || false,
            openButtonVisible: !!openButton && getComputedStyle(openButton).display !== 'none' && getComputedStyle(openButton).visibility !== 'hidden',
            openButtonOnClick: typeof openButton?.onclick,
            layerHidden: layer?.hidden ?? null,
            sourceLabel: document.querySelector('#handoffSourceLabel')?.textContent || '',
            toast: document.querySelector('#toast')?.textContent || '',
          };
        }).catch(e => ({ diagnosticError: String(e) }));
        throw new Error(`等待接续面板打开超时：${JSON.stringify(state)}`);
      }
      const info = await assertTauriPage(main, '主窗口内接续面板');
      const text = await visibleText(main);
      const screenshot = await saveScreenshot(main, 'handoff-inline-panel.png');
      await main.locator('#handoffClose').click({ force: true, timeout: 3000 }).catch(() => {});

      return {
        ...info,
        模式: '主窗口内接续面板',
        可见文本片段: text.slice(0, 180),
        截图: screenshot,
      };
    }
  }
  await handoff.waitForLoadState('domcontentloaded');
  const info = await assertTauriPage(handoff, '接续面板窗口');
  const text = await visibleText(handoff);
  const screenshot = await saveScreenshot(handoff, 'handoff-window.png');
  const inlineState = await main.evaluate(() => {
    const layer = document.querySelector('#handoffLayer');
    return { hidden: layer?.hidden ?? null };
  }).catch(error => ({ error: String(error) }));
  if (inlineState.hidden === false) {
    throw new Error('Native handoff panel opened, but the main-window inline handoff panel is still visible.');
  }

  return {
    ...info,
    可见文本片段: text.slice(0, 180),
    截图: screenshot,
  };
}

async function runFilteringFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');
  await ensureWatcherSurface(page);
  await waitForSessionSurface(page);
  await resetMainFilters(page);

  const initial = await collectFilteringSnapshot(page);
  const layout = await measureMainFilterLayout(page);
  if (!layout.dropdownSameWidth || !layout.searchAligned || !layout.perRowAligned || !layout.providerPopoverWideEnough || !layout.workspacePopoverWideEnough) {
    throw new Error(`主面板筛选控件布局不符合预期：${JSON.stringify(layout)}`);
  }

  const providerOrder = ['codex', 'opencode', 'copilot', 'claude'];
  const targetProvider = providerOrder.find(provider => (initial.providerCounts[provider] || 0) > 0)
    || Object.entries(initial.providerCounts).find(([, count]) => count > 0)?.[0]
    || '';
  const providerResult = {
    跳过: '',
    provider: targetProvider,
  };

  if (!targetProvider) {
    providerResult.跳过 = '当前真实数据没有可用于 provider 筛选的会话卡片。';
  } else {
    await selectProviderFilter(page, targetProvider);
    await page.locator('.segment[data-status="all"]').click();
    await page.waitForFunction((provider) => {
      const visible = Array.from(document.querySelectorAll('.lane .session-card:not([hidden])'));
      return visible.length > 0 && visible.every(card => card.dataset.provider === provider);
    }, targetProvider, { timeout: 10000 });

    const afterProvider = await collectFilteringSnapshot(page);
    const expectedProviderTotal = initial.providerCounts[targetProvider] || 0;
    if (afterProvider.sessionTotal !== expectedProviderTotal) {
      throw new Error(`provider 筛选计数不一致：expected ${expectedProviderTotal}, actual ${afterProvider.sessionTotal}`);
    }
    providerResult.providerLabel = afterProvider.providerLabel;
    providerResult.total = afterProvider.sessionTotal;
    providerResult.visibleProviders = afterProvider.visibleProviders;

    const searchableTitle = afterProvider.visibleCards.find(card => card.title.length >= 3)?.title || afterProvider.visibleCards[0]?.title || '';
    const searchQuery = searchToken(searchableTitle);
    if (!searchQuery) {
      providerResult.搜索 = { 跳过: '目标 provider 下没有可搜索标题。' };
    } else {
      await page.locator('#sessionSearch').fill(searchQuery);
      await page.waitForFunction(([provider, query]) => {
        const normalizedQuery = String(query).toLowerCase();
        const visible = Array.from(document.querySelectorAll('.lane .session-card:not([hidden])'));
        return visible.length > 0
          && visible.every(card => card.dataset.provider === provider)
          && visible.every(card => ((card.dataset.sessionTitle || card.querySelector('.session-title')?.textContent || '').toLowerCase()).includes(normalizedQuery));
      }, [targetProvider, searchQuery], { timeout: 10000 });
      const afterSearch = await collectFilteringSnapshot(page);
      const expectedSearchTotal = initial.cards
        .filter(card => card.provider === targetProvider && card.title.toLowerCase().includes(searchQuery.toLowerCase()))
        .length;
      if (afterSearch.sessionTotal !== expectedSearchTotal) {
        throw new Error(`标题搜索计数不一致：expected ${expectedSearchTotal}, actual ${afterSearch.sessionTotal}`);
      }

      const emptyQuery = `aw-no-match-${Date.now()}`;
      await page.locator('#sessionSearch').fill(emptyQuery);
      await page.waitForFunction(() => {
        const emptyState = document.querySelector('#sessionEmptyState');
        return emptyState && !emptyState.hidden && document.querySelector('#sessionTotal')?.textContent.trim() === '0';
      }, null, { timeout: 10000 });
      const emptyState = await page.evaluate(() => ({
        title: document.querySelector('#sessionEmptyTitle')?.textContent || '',
        text: document.querySelector('#sessionEmptyText')?.textContent || '',
        clear: document.querySelector('#clearSessionFilters')?.textContent || '',
      }));

      await page.locator('#clearSessionFilters').click();
      await page.waitForFunction(() => {
        const activeStatus = document.querySelector('.segment[data-status].active')?.dataset.status || '';
        return activeStatus === 'all'
          && !document.querySelector('#sessionSearch')?.value
          && document.querySelector('#providerPickerValue')?.textContent.trim();
      }, null, { timeout: 10000 });
      const afterClear = await collectFilteringSnapshot(page);
      if (afterClear.sessionTotal !== initial.sessionTotal) {
        throw new Error(`清除筛选后总数未恢复：expected ${initial.sessionTotal}, actual ${afterClear.sessionTotal}`);
      }

      providerResult.搜索 = {
        query: searchQuery,
        total: afterSearch.sessionTotal,
        空状态: emptyState,
        清除后: {
          providerLabel: afterClear.providerLabel,
          activeStatus: afterClear.activeStatus,
          searchValue: afterClear.searchValue,
          total: afterClear.sessionTotal,
        },
      };
    }
  }

  await resetMainFilters(page);
  const todoScope = await verifyTodoWorkspaceScope(page);
  const screenshot = await saveScreenshot(page, 'filtering-main-panel.png');

  return {
    初始: {
      total: initial.sessionTotal,
      providerCounts: initial.providerCounts,
      workspaceLabel: initial.workspaceLabel,
    },
    布局: layout,
    Provider与搜索: providerResult,
    Todo按Workspace过滤: todoScope,
    截图: screenshot,
  };
}

async function runOpenCodeSessionFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');

  const includeOpenCode = page.locator('#includeOpenCode');
  if (await includeOpenCode.count()) {
    const enabled = await includeOpenCode.isChecked().catch(() => true);
    if (!enabled) {
      await page.locator('#settingsToggle').click();
      await includeOpenCode.check();
      await page.waitForTimeout(500);
    }
  }

  await page.waitForFunction(() => {
    return Array.from(document.querySelectorAll('.lane .session-card'))
      .some(card => card.dataset.provider === 'opencode');
  }, null, { timeout: 30000 });

  const card = page.locator('.lane .session-card[data-provider="opencode"]').first();
  await card.scrollIntoViewIfNeeded();
  const cardInfo = await card.evaluate(element => ({
    sessionId: element.dataset.sessionId || '',
    workspace: element.dataset.workspaceLabel || element.dataset.workspace || '',
    workspacePath: element.dataset.workspacePath || element.dataset.open || '',
    status: element.dataset.status || '',
    title: element.dataset.sessionTitle || element.querySelector('.session-title')?.textContent || '',
    文本片段: element.innerText.slice(0, 180),
  }));
  await page.locator('#toast').evaluate(element => {
    element.textContent = '';
    element.classList.remove('show', 'error');
  });
  const beforeToast = '';

  await card.click();
  await page.waitForFunction((previous) => {
    const text = document.querySelector('#toast')?.textContent || '';
    return text && text !== previous && !/正在打开会话|Opening session/i.test(text);
  }, beforeToast, { timeout: 30000 });

  const toastText = await page.locator('#toast').textContent().catch(() => '');
  if (/failed|失败|not found|找不到|fallback failed|不支持|cannot open an existing session/i.test(toastText)) {
    throw new Error(`OpenCode 卡片点击返回错误：${toastText}`);
  }
  const screenshot = await saveScreenshot(page, 'opencode-card-after-click.png');

  return {
    点击目标: '.lane .session-card[data-provider="opencode"]',
    卡片: cardInfo,
    Toast: toastText,
    OpenCodeWeb旁路: /已打开会话|Session opened/i.test(toastText),
    截图: screenshot,
  };
}

async function runOpenCodeHandoffContextFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');

  const includeOpenCode = page.locator('#includeOpenCode');
  if (await includeOpenCode.count()) {
    const enabled = await includeOpenCode.isChecked().catch(() => true);
    if (!enabled) {
      await page.locator('#settingsToggle').click();
      await includeOpenCode.check();
      await page.waitForTimeout(500);
    }
  }

  await page.waitForFunction(() => {
    return Array.from(document.querySelectorAll('.lane .session-card'))
      .some(card => card.dataset.provider === 'opencode');
  }, null, { timeout: 30000 });

  const card = page.locator('.lane .session-card[data-provider="opencode"]').first();
  await card.scrollIntoViewIfNeeded();
  const cardInfo = await card.evaluate(element => ({
    sessionId: element.dataset.sessionId || '',
    workspace: element.dataset.workspaceLabel || element.dataset.workspace || '',
    workspacePath: element.dataset.workspacePath || element.dataset.open || '',
    title: element.dataset.sessionTitle || element.querySelector('.session-title')?.textContent || '',
  }));

  await card.click({ button: 'right' });
  await page.locator('#handoffOpen').click();

  const handoff = await waitForPage(
    candidate => candidate.url().includes('?handoff=1'),
    '接续面板窗口',
    20000,
  );
  await handoff.waitForLoadState('domcontentloaded');
  await assertTauriPage(handoff, '接续面板窗口');
  await handoff.waitForFunction(() => {
    const prompt = document.querySelector('#handoffPromptPreview')?.value || '';
    return /handoff-sources[\\/]+opencode/i.test(prompt)
      && /ses_[A-Za-z0-9]/.test(prompt)
      && !/主要来源文件:\s*（无）|Primary source file:\s*\(none\)/i.test(prompt);
  }, null, { timeout: 30000 });

  const prompt = await handoff.locator('#handoffPromptPreview').inputValue();
  const sourcePathMatch = prompt.match(/(?:主要来源文件|Primary source file):\s*(.+\.md)/);
  const sourcePath = sourcePathMatch ? sourcePathMatch[1].trim() : '';
  if (!sourcePath || !fs.existsSync(sourcePath)) {
    throw new Error(`OpenCode 接续来源文件不存在：${sourcePath || '(未从 prompt 解析到)'}`);
  }
  const sourceText = fs.readFileSync(sourcePath, 'utf8');
  if (!sourceText.includes('AgentWatcher OpenCode Source Session') || !sourceText.includes('## Transcript')) {
    throw new Error(`OpenCode 接续来源文件格式不完整：${sourcePath}`);
  }
  if (!/###\s+(user|assistant)\b/i.test(sourceText)) {
    throw new Error(`OpenCode 接续来源文件没有可读用户/助手回合：${sourcePath}`);
  }
  const modeResults = [];
  for (const mode of ['agents', 'code-chat', 'claude-panel', 'codex-app', 'opencode-cli']) {
    await handoff.locator('#handoffProviderButton').click();
    await handoff.locator(`#handoffProviderOptions [data-mode="${mode}"]`).click();
    await handoff.waitForFunction((expectedSourcePath) => {
      const prompt = document.querySelector('#handoffPromptPreview')?.value || '';
      return prompt.includes(expectedSourcePath);
    }, sourcePath, { timeout: 10000 });
    const modePrompt = await handoff.locator('#handoffPromptPreview').inputValue();
    if (!modePrompt.includes(sourcePath)) {
      throw new Error(`切换目标 ${mode} 后 Prompt 丢失 OpenCode 来源文件路径。`);
    }
    modeResults.push({
      mode,
      targetLine: (modePrompt.match(/目标代理:\s*(.+)|Target agent:\s*(.+)/) || [])[0] || '',
    });
  }
  const screenshot = await saveScreenshot(handoff, 'opencode-handoff-context.png');

  return {
    卡片: cardInfo,
    Prompt包含来源文件: true,
    已验证目标模式: modeResults,
    来源文件: sourcePath,
    来源文件大小: sourceText.length,
    Prompt片段: prompt.slice(0, 800),
    截图: screenshot,
  };
}

async function runSettingsPerformanceFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');

  const perfToggle = page.locator('#performanceToggle');
  if (await perfToggle.count()) {
    const pressed = await perfToggle.getAttribute('aria-pressed').catch(() => 'false');
    if (pressed !== 'true') await perfToggle.click();
  }
  const performancePage = await waitForPage(
    candidate => candidate.url().includes('?performance=1'),
    '性能诊断窗口',
    20000,
  );
  await assertTauriPage(performancePage, '性能诊断窗口');

  let handoff = context.pages().find(candidate => candidate.url().includes('?handoff=1'));
  if (!handoff) {
    await ensureFirstOpenCodeHandoffOpen(page);
    handoff = await waitForPage(
      candidate => candidate.url().includes('?handoff=1'),
      '接续面板窗口',
      20000,
    );
  }
  await assertTauriPage(handoff, '接续面板窗口');

  const currentLang = await page.evaluate(() => document.body.dataset.lang || 'en');
  const firstTarget = currentLang === 'zh' ? 'en' : 'zh';
  const secondTarget = currentLang === 'zh' ? 'zh' : 'en';
  const first = await measureLanguageSwitch(page, [handoff, performancePage], firstTarget);
  const second = await measureLanguageSwitch(page, [handoff, performancePage], secondTarget);
  const maxMs = Math.max(first.ms, second.ms);
  if (maxMs > 2000) {
    throw new Error(`设置语言切换仍然过慢：${maxMs}ms`);
  }

  const screenshot = await saveScreenshot(page, 'settings-performance-after-switch.png');
  return {
    子面板: ['接续面板窗口', '性能诊断窗口'],
    切换: [first, second],
    最大耗时毫秒: maxMs,
    阈值毫秒: 2000,
    截图: screenshot,
  };
}

async function ensureWatcherSurface(page) {
  const railOnly = await page.evaluate(() => document.body.classList.contains('is-rail-only')).catch(() => false);
  if (railOnly) {
    await page.locator('.rail-logo').click({ force: true, timeout: 5000 });
    await page.waitForFunction(() => !document.body.classList.contains('is-rail-only'), null, { timeout: 10000 });
    await page.waitForFunction(() => {
      const rect = document.querySelector('.watcher-window')?.getBoundingClientRect();
      return rect && rect.width > 300 && rect.height > 260;
    }, null, { timeout: 10000 }).catch(() => {});
  }

  const inAgentTaskMode = await page.evaluate(() => document.body.classList.contains('is-agent-task-mode')).catch(() => false);
  if (inAgentTaskMode) {
    await page.locator('#todoToggle').click({ force: true });
    await page.waitForFunction(() => !document.body.classList.contains('is-agent-task-mode'), null, { timeout: 10000 });
  }

  const perfPressed = await page.locator('#performanceToggle').getAttribute('aria-pressed').catch(() => 'false');
  const perfVisible = await page.locator('#perfPanel').evaluate(element => {
    if (!element || element.hidden) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
  }).catch(() => false);
  if (perfPressed === 'true' || perfVisible) {
    await page.locator('#performanceToggle').click({ force: true });
    await page.waitForFunction(() => {
      const panel = document.querySelector('#perfPanel');
      return document.querySelector('#performanceToggle')?.getAttribute('aria-pressed') === 'false'
        && (!panel || panel.hidden || window.getComputedStyle(panel).display === 'none');
    }, null, { timeout: 10000 });
  }

  const settingsVisible = await page.locator('#settingsPanel').evaluate(element => {
    if (!element || element.hidden) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
  }).catch(() => false);
  if (settingsVisible) {
    await page.locator('#settingsToggle').click();
    await page.waitForFunction(() => document.querySelector('#settingsPanel')?.hidden === true, null, { timeout: 10000 });
  }
}

async function waitForSessionSurface(page) {
  await page.waitForFunction(() => {
    return document.querySelector('#workspacePickerButton')
      && document.querySelector('#providerPickerButton')
      && document.querySelector('#sessionSearch')
      && document.querySelector('#sessionTotal')
      && document.querySelector('.segment[data-status="all"]');
  }, null, { timeout: 20000 });
}

async function resetMainFilters(page) {
  await ensureWatcherSurface(page);
  await waitForSessionSurface(page);
  await page.locator('.segment[data-status="all"]').click();
  await page.locator('#sessionSearch').fill('');

  const providerLabel = await page.locator('#providerPickerValue').textContent().catch(() => '');
  if (!/全部提供方|All providers/i.test(providerLabel || '')) {
    await selectProviderFilter(page, 'all');
  }

  const workspaceLabel = await page.locator('#workspacePickerValue').textContent().catch(() => '');
  if (!/全部工作区|All workspaces/i.test(workspaceLabel || '')) {
    await selectWorkspaceFilter(page, '');
  }

  await page.waitForFunction(() => {
    const activeStatus = document.querySelector('.segment[data-status].active')?.dataset.status || '';
    const provider = document.querySelector('#providerPickerValue')?.textContent || '';
    const workspace = document.querySelector('#workspacePickerValue')?.textContent || '';
    return activeStatus === 'all'
      && !document.querySelector('#sessionSearch')?.value
      && /全部提供方|All providers/i.test(provider)
      && /全部工作区|All workspaces/i.test(workspace);
  }, null, { timeout: 10000 });
}

async function selectProviderFilter(page, provider) {
  await domClick(page, '#providerPickerButton');
  const selector = `#providerPickerOptions .provider-picker-option[data-provider="${cssAttr(provider)}"]`;
  await page.waitForFunction((nextSelector) => Boolean(document.querySelector(nextSelector)), selector, { timeout: 5000 });
  await page.evaluate((nextSelector) => document.querySelector(nextSelector)?.click(), selector);
}

async function selectWorkspaceFilter(page, workspaceKey) {
  await domClick(page, '#workspacePickerButton');
  const selector = `#workspacePickerOptions .workspace-picker-option[data-workspace-key="${cssAttr(workspaceKey)}"]`;
  await page.waitForFunction((nextSelector) => Boolean(document.querySelector(nextSelector)), selector, { timeout: 5000 });
  await page.evaluate((nextSelector) => {
    const option = document.querySelector(nextSelector);
    option?.scrollIntoView({ block: 'nearest' });
    option?.click();
  }, selector);
}

async function domClick(page, selector) {
  await page.waitForFunction((nextSelector) => {
    const element = document.querySelector(nextSelector);
    return element && !element.disabled;
  }, selector, { timeout: 5000 });
  await page.evaluate((nextSelector) => document.querySelector(nextSelector)?.click(), selector);
}

function cssAttr(value) {
  return String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"');
}

function searchToken(title) {
  const trimmed = String(title || '').trim();
  const word = trimmed.split(/\s+/).find(part => part.replace(/[^\p{L}\p{N}_-]/gu, '').length >= 3);
  return (word || trimmed).slice(0, 18);
}

async function collectFilteringSnapshot(page) {
  return page.evaluate(() => {
    const cards = Array.from(document.querySelectorAll('.lane .session-card')).map(card => ({
      provider: card.dataset.provider || '',
      status: card.dataset.status || '',
      title: (card.dataset.sessionTitle || card.querySelector('.session-title')?.textContent || '').trim(),
      workspaceKey: card.dataset.workspaceKey || '',
      workspaceLabel: card.dataset.workspaceLabel || card.dataset.workspace || '',
      hidden: card.hidden,
    }));
    const providerCounts = {};
    for (const card of cards) {
      if (!card.provider) continue;
      providerCounts[card.provider] = (providerCounts[card.provider] || 0) + 1;
    }
    const visibleCards = cards.filter(card => !card.hidden);
    return {
      cards,
      visibleCards,
      providerCounts,
      visibleProviders: [...new Set(visibleCards.map(card => card.provider).filter(Boolean))],
      sessionTotal: Number(document.querySelector('#sessionTotal')?.textContent?.trim() || '0'),
      visibleNow: Number(document.querySelector('#visibleNow')?.textContent?.trim() || '0'),
      queueNow: Number(document.querySelector('#queueNow')?.textContent?.trim() || '0'),
      activeStatus: document.querySelector('.segment[data-status].active')?.dataset.status || '',
      providerLabel: document.querySelector('#providerPickerValue')?.textContent?.trim() || '',
      workspaceLabel: document.querySelector('#workspacePickerValue')?.textContent?.trim() || '',
      searchValue: document.querySelector('#sessionSearch')?.value || '',
      statusLabels: [...document.querySelectorAll('.segment[data-status]')].map(segment => ({
        status: segment.dataset.status,
        text: segment.textContent.trim().replace(/\s+/g, ' '),
      })),
    };
  });
}

async function measureMainFilterLayout(page) {
  const workspacePopoverWidth = await measurePopoverWidth(page, '#workspacePickerButton', '#workspacePickerPopover');
  const providerPopoverWidth = await measurePopoverWidth(page, '#providerPickerButton', '#providerPickerPopover');
  return page.evaluate(([workspacePopoverWidth, providerPopoverWidth]) => {
    const rect = (selector) => {
      const element = document.querySelector(selector);
      if (!element) return null;
      const box = element.getBoundingClientRect();
      return {
        left: Math.round(box.left),
        right: Math.round(box.right),
        width: Math.round(box.width),
      };
    };
    const workspace = rect('#workspacePickerButton');
    const provider = rect('#providerPickerButton');
    const search = rect('#sessionSearch');
    const perRow = rect('#sizePreset');
    const controlStrip = document.querySelector('.control-strip');
    const watcher = document.querySelector('.watcher-window');
    return {
      workspaceButtonWidth: workspace?.width || 0,
      providerButtonWidth: provider?.width || 0,
      workspacePopoverWidth,
      providerPopoverWidth,
      dropdownSameWidth: Math.abs((workspace?.width || 0) - (provider?.width || 0)) <= 1,
      searchAligned: Math.abs((search?.left || 0) - (workspace?.left || 0)) <= 1,
      perRowAligned: Math.abs((perRow?.left || 0) - (workspace?.left || 0)) <= 1,
      providerPopoverWideEnough: providerPopoverWidth >= 220,
      workspacePopoverWideEnough: workspacePopoverWidth >= 280,
      controlStripFits: !controlStrip || controlStrip.scrollWidth <= controlStrip.clientWidth + 1,
      watcherFits: !watcher || watcher.scrollWidth <= watcher.clientWidth + 1,
    };
  }, [workspacePopoverWidth, providerPopoverWidth]);
}

async function measurePopoverWidth(page, buttonSelector, popoverSelector) {
  if (!(await page.locator(popoverSelector).evaluate(element => element && !element.hidden).catch(() => false))) {
    await domClick(page, buttonSelector);
  }
  await page.locator(popoverSelector).waitFor({ state: 'visible', timeout: 5000 });
  const width = await page.locator(popoverSelector).evaluate(element => Math.round(element.getBoundingClientRect().width));
  await domClick(page, buttonSelector);
  await page.waitForFunction((selector) => document.querySelector(selector)?.hidden === true, popoverSelector, { timeout: 5000 }).catch(() => {});
  return width;
}

async function verifyTodoWorkspaceScope(page) {
  await resetMainFilters(page);
  await waitForReadyTodoNudge(page);
  let allWorkspaceTodo = await readTodoNudge(page);
  let fixture = null;
  if (allWorkspaceTodo.hidden || !allWorkspaceTodo.workspaceKey) {
    fixture = await installTodoScopeFixture(page);
    if (!fixture) {
      return {
        跳过: '当前真实 todo 队列没有 ready nudge，且没有足够 workspace 生成临时可恢复 fixture。',
        AllWorkspaces: allWorkspaceTodo,
      };
    }
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => document.body.dataset.lang === 'zh', null, { timeout: 10000 }).catch(() => {});
    await resetMainFilters(page);
    await waitForReadyTodoNudge(page, 'Realtest workspace-scoped ready todo', 10000);
    allWorkspaceTodo = await readTodoNudge(page);
  }

  if (allWorkspaceTodo.hidden || !allWorkspaceTodo.workspaceKey) {
    if (fixture) restoreTodoScopeFixture(fixture);
    throw new Error('临时 todo fixture 写入后仍未显示 ready nudge。');
  }

  try {
    await domClick(page, '#workspacePickerButton');
    const workspaceKeys = await page.evaluate(() => Array.from(document.querySelectorAll('#workspacePickerOptions .workspace-picker-option'))
      .map(option => option.dataset.workspaceKey || '')
      .filter(Boolean));
    await domClick(page, '#workspacePickerButton');

    const todoWorkspaceSelectable = workspaceKeys.includes(allWorkspaceTodo.workspaceKey);
    const otherWorkspaceKey = workspaceKeys.find(key => key !== allWorkspaceTodo.workspaceKey && key !== fixture?.targetKey)
      || fixture?.otherKey
      || workspaceKeys.find(key => key !== allWorkspaceTodo.workspaceKey);
    if (!otherWorkspaceKey) {
      return {
        跳过: '当前只有 ready todo 所属 workspace，没有可对照的其它 workspace。',
        AllWorkspaces: allWorkspaceTodo,
        临时Fixture: fixture ? { 已使用: true, targetKey: fixture.targetKey } : { 已使用: false },
      };
    }

    await selectWorkspaceFilter(page, otherWorkspaceKey);
    await page.waitForFunction(() => document.querySelector('#todoNudge')?.hidden === true, null, { timeout: 10000 });
    const otherWorkspaceTodo = await page.evaluate(() => ({
      workspace: document.querySelector('#workspacePickerValue')?.textContent?.trim() || '',
      hidden: document.querySelector('#todoNudge')?.hidden ?? false,
    }));

    let scopedWorkspaceTodo = {
      跳过: 'ready todo 所属 workspace 当前不在主面板 workspace 下拉中，无法直接切回该 workspace。',
      workspaceKey: allWorkspaceTodo.workspaceKey,
    };
    let selectedTodoWorkspace = '';
    if (todoWorkspaceSelectable) {
      await selectWorkspaceFilter(page, allWorkspaceTodo.workspaceKey);
      await page.waitForFunction((workspaceKey) => {
        const nudge = document.querySelector('#todoNudge');
        const key = document.querySelector('#todoNudgeOpen')?.dataset.workspaceKey || '';
        return nudge && !nudge.hidden && key === workspaceKey;
      }, allWorkspaceTodo.workspaceKey, { timeout: 10000 });
      scopedWorkspaceTodo = await readTodoNudge(page);
      selectedTodoWorkspace = await page.locator('#workspacePickerValue').textContent().catch(() => '');
    }

    await selectWorkspaceFilter(page, '');
    return {
      AllWorkspaces: allWorkspaceTodo,
      OtherWorkspace: otherWorkspaceTodo,
      TodoWorkspace: {
        ...scopedWorkspaceTodo,
        workspace: selectedTodoWorkspace.trim(),
      },
      TodoWorkspaceSelectable: todoWorkspaceSelectable,
      临时Fixture: fixture ? { 已使用: true, targetKey: fixture.targetKey, otherKey: fixture.otherKey } : { 已使用: false },
    };
  } finally {
    if (fixture) {
      restoreTodoScopeFixture(fixture);
      await page.reload({ waitUntil: 'domcontentloaded' }).catch(() => {});
    }
  }
}

async function readTodoNudge(page) {
  return page.evaluate(() => ({
    hidden: document.querySelector('#todoNudge')?.hidden ?? true,
    title: document.querySelector('#todoNudgeTitle')?.textContent?.trim() || '',
    task: document.querySelector('#todoNudgeTask')?.textContent?.trim() || '',
    workspaceKey: document.querySelector('#todoNudgeOpen')?.dataset.workspaceKey || '',
  }));
}

async function selectAgentTaskWorkspace(page, workspaceKey) {
  await page.locator('#agentTaskWorkspaceButton').click();
  await page.waitForFunction(() => !document.querySelector('#agentTaskWorkspacePopover')?.hidden, null, { timeout: 5000 });
  await page.evaluate((key) => {
    const options = [...document.querySelectorAll('#agentTaskWorkspaceOptions [data-workspace-key]')];
    const option = options.find(item => item.dataset.workspaceKey === key) || options[0];
    option?.click();
  }, workspaceKey);
  await page.waitForFunction((key) => {
    const popoverClosed = document.querySelector('#agentTaskWorkspacePopover')?.hidden;
    const selected = document.querySelector('#agentTaskWorkspaceValue')?.textContent || '';
    return popoverClosed && (!!selected || !!key);
  }, workspaceKey, { timeout: 5000 });
}

function snapshotTodoStateFile() {
  const appData = process.env.APPDATA;
  if (!appData) return null;
  const todoFile = path.join(appData, 'AgentWatcher', 'todos.v1.json');
  return {
    path: todoFile,
    existed: fs.existsSync(todoFile),
    text: fs.existsSync(todoFile) ? fs.readFileSync(todoFile, 'utf8') : '',
  };
}

function restoreTodoStateFile(backup) {
  if (!backup?.path) return;
  if (backup.existed) {
    fs.mkdirSync(path.dirname(backup.path), { recursive: true });
    fs.writeFileSync(backup.path, backup.text, 'utf8');
  } else if (fs.existsSync(backup.path)) {
    fs.rmSync(backup.path, { force: true });
  }
}

function readTodoTaskSnapshot(taskId) {
  const appData = process.env.APPDATA;
  if (!appData || !taskId) return null;
  const todoFile = path.join(appData, 'AgentWatcher', 'todos.v1.json');
  if (!fs.existsSync(todoFile)) return null;
  const state = safeJsonParse(fs.readFileSync(todoFile, 'utf8'), { workspaces: {} });
  for (const workspace of Object.values(state.workspaces || {})) {
    const task = (workspace?.tasks || []).find(item => item?.id === taskId);
    if (!task) continue;
    const details = task.pluginWorkflowExecution?.result || task.pluginWorkflowPlan?.result || task.execution?.result || {};
    const artifacts = Array.isArray(task.execution?.result?.artifacts) ? task.execution.result.artifacts : [];
    const latestDispatch = Array.isArray(task.dispatches) && task.dispatches.length
      ? task.dispatches[task.dispatches.length - 1]
      : null;
    return {
      id: task.id,
      workflow: task.workflow || '',
      launchMode: task.launchMode || '',
      workspacePath: workspace?.path || '',
      taskStatus: task.status || '',
      dispatches: Array.isArray(task.dispatches) ? task.dispatches.length : -1,
      dispatchWorkspacePath: latestDispatch?.workspacePath || '',
      dispatchProviderWorkspacePath: latestDispatch?.providerWorkspacePath || '',
      lifecyclePhase: task.pluginWorkflowLifecycle?.phase || '',
      lifecycleMessage: task.pluginWorkflowLifecycle?.message || '',
      lifecycleEvents: Array.isArray(task.pluginWorkflowLifecycle?.events) ? task.pluginWorkflowLifecycle.events.length : -1,
      executionStatus: task.execution?.status || '',
      executionCommand: task.execution?.commandName || '',
      executionKind: details.kind || '',
      resultStatus: task.execution?.result?.status || '',
      detailsStatus: details.status || '',
      detailsCommand: details.command || '',
      requestTaskId: details.request?.taskId || '',
      requestWorkspace: details.request?.workspace || '',
      stages: Array.isArray(details.stages) ? details.stages.length : -1,
      blockedActions: Array.isArray(details.blockedActions) ? details.blockedActions.length : -1,
      artifacts: artifacts.length,
      artifactLabels: artifacts.map(item => String(item?.label || '')).filter(Boolean),
      provider: details.providerPackage?.provider || details.request?.provider || '',
      knowledgeClosure: details.knowledgeClosure || '',
      sideEffects: Array.isArray(details.sideEffects) ? details.sideEffects.length : -1,
    };
  }
  return null;
}

async function waitForTodoTaskSnapshot(taskId, predicate, timeoutMs = 30000) {
  const started = Date.now();
  let latest = null;
  while (Date.now() - started < timeoutMs) {
    latest = readTodoTaskSnapshot(taskId);
    if (latest && predicate(latest)) return latest;
    await delay(500);
  }
  return latest;
}

async function waitForReadyTodoNudge(page, taskTitle = '', timeout = 5000) {
  await page.waitForFunction((expectedTitle) => {
    const nudge = document.querySelector('#todoNudge');
    const task = document.querySelector('#todoNudgeTask')?.textContent || '';
    return nudge && !nudge.hidden && (!expectedTitle || task.includes(expectedTitle));
  }, taskTitle, { timeout }).catch(() => {});
}

async function installTodoScopeFixture(page) {
  const appData = process.env.APPDATA;
  if (!appData) return null;
  const todoFile = path.join(appData, 'AgentWatcher', 'todos.v1.json');
  const backup = {
    path: todoFile,
    existed: fs.existsSync(todoFile),
    text: fs.existsSync(todoFile) ? fs.readFileSync(todoFile, 'utf8') : '',
  };
  const original = backup.existed ? safeJsonParse(backup.text, { version: 1, workspaces: {} }) : { version: 1, workspaces: {} };
  const state = JSON.parse(JSON.stringify(original && typeof original === 'object' ? original : { version: 1, workspaces: {} }));
  state.version = 1;
  state.workspaces = state.workspaces && typeof state.workspaces === 'object' ? state.workspaces : {};

  const workspaces = await page.evaluate(() => {
    const byKey = new Map();
    document.querySelectorAll('.lane .session-card').forEach(card => {
      const key = card.dataset.workspaceKey || '';
      if (!key) return;
      const existing = byKey.get(key) || {
        key,
        label: card.dataset.workspaceLabel || card.dataset.workspace || key,
        path: card.dataset.workspacePath || '',
        hasActiveSession: false,
      };
      existing.label = existing.label || card.dataset.workspaceLabel || card.dataset.workspace || key;
      existing.path = existing.path || card.dataset.workspacePath || '';
      existing.hasActiveSession = existing.hasActiveSession || ['waiting', 'running'].includes(card.dataset.status || '');
      byKey.set(key, existing);
    });
    return [...byKey.values()];
  });
  const hasReady = (workspaceKey) => {
    const tasks = state.workspaces?.[workspaceKey]?.tasks;
    return Array.isArray(tasks) && tasks.some(task => task?.status === 'ready');
  };
  const canShowNudge = (workspace) => !workspace.hasActiveSession;
  const target = workspaces.find(workspace => canShowNudge(workspace) && workspace.path && workspaces.some(other => other.key !== workspace.key && !hasReady(other.key)))
    || workspaces.find(workspace => canShowNudge(workspace) && workspaces.some(other => other.key !== workspace.key && !hasReady(other.key)));
  if (!target) return null;
  const other = workspaces.find(workspace => workspace.key !== target.key && !hasReady(workspace.key))
    || workspaces.find(workspace => workspace.key !== target.key);
  if (!other) return null;

  const fixtureTaskId = `realtest-filtering-ready-${runId}`;
  for (const workspace of Object.values(state.workspaces)) {
    if (Array.isArray(workspace?.tasks)) {
      workspace.tasks = workspace.tasks.filter(task => task?.id !== fixtureTaskId);
    }
  }
  const workspaceState = state.workspaces[target.key] || {
    key: target.key,
    label: target.label,
    path: target.path,
    tasks: [],
  };
  workspaceState.key = target.key;
  workspaceState.label = target.label || workspaceState.label || target.key;
  workspaceState.path = target.path || workspaceState.path || '';
  workspaceState.tasks = Array.isArray(workspaceState.tasks) ? workspaceState.tasks : [];
  workspaceState.tasks.unshift({
    id: fixtureTaskId,
    title: 'Realtest workspace-scoped ready todo',
    description: 'Temporary task inserted by tauri-realtest filtering flow.',
    acceptance: 'Todo nudge appears only for all workspaces or this workspace.',
    status: 'ready',
    order: 0,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    dispatches: [],
  });
  workspaceState.tasks.forEach((task, index) => { task.order = index; });
  state.workspaces[target.key] = workspaceState;

  fs.mkdirSync(path.dirname(todoFile), { recursive: true });
  fs.writeFileSync(todoFile, JSON.stringify(state, null, 2), 'utf8');
  return {
    ...backup,
    targetKey: target.key,
    otherKey: other.key,
    taskId: fixtureTaskId,
  };
}

function restoreTodoScopeFixture(fixture) {
  if (!fixture?.path) return;
  if (fixture.existed) {
    fs.mkdirSync(path.dirname(fixture.path), { recursive: true });
    fs.writeFileSync(fixture.path, fixture.text, 'utf8');
  } else if (fs.existsSync(fixture.path)) {
    fs.rmSync(fixture.path, { force: true });
  }
}

function safeJsonParse(text, fallback) {
  try {
    return JSON.parse(text);
  } catch {
    return fallback;
  }
}

async function ensureFirstOpenCodeHandoffOpen(page) {
  const includeOpenCode = page.locator('#includeOpenCode');
  if (await includeOpenCode.count()) {
    const enabled = await includeOpenCode.isChecked().catch(() => true);
    if (!enabled) {
      await page.locator('#settingsToggle').click();
      await includeOpenCode.check();
      await page.waitForTimeout(500);
    }
  }
  await page.waitForFunction(() => {
    return Array.from(document.querySelectorAll('.lane .session-card'))
      .some(card => card.dataset.provider === 'opencode');
  }, null, { timeout: 30000 });
  const card = page.locator('.lane .session-card[data-provider="opencode"]').first();
  await card.scrollIntoViewIfNeeded();
  await card.click({ button: 'right' });
  await page.locator('#handoffOpen').click();
}

async function measureLanguageSwitch(page, childPages, lang) {
  await openSettingsPanel(page);
  const expectedHtmlLang = lang === 'zh' ? 'zh-CN' : 'en';
  const start = Date.now();
  await page.locator(`[data-setting-lang="${lang}"]`).click();
  await page.waitForFunction(([htmlLang, bodyLang]) => (
    document.documentElement.lang === htmlLang && document.body.dataset.lang === bodyLang
  ), [expectedHtmlLang, lang], { timeout: 5000 });
  for (const child of childPages) {
    await child.waitForFunction(([htmlLang, bodyLang]) => (
      document.documentElement.lang === htmlLang && document.body.dataset.lang === bodyLang
    ), [expectedHtmlLang, lang], { timeout: 5000 });
  }
  return {
    lang,
    ms: Date.now() - start,
    主窗口: await page.evaluate(() => ({
      htmlLang: document.documentElement.lang,
      bodyLang: document.body.dataset.lang || '',
    })),
    子窗口: await Promise.all(childPages.map(child => child.evaluate(() => ({
      title: document.title,
      htmlLang: document.documentElement.lang,
      bodyLang: document.body.dataset.lang || '',
    })))),
  };
}

async function openSettingsPanel(page) {
  await ensureWatcherSurface(page);
  const visible = await page.locator('#settingsPanel').evaluate(element => {
    if (!element) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return !element.hidden && rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
  }).catch(() => false);
  if (!visible) {
    await page.locator('#settingsToggle').click({ force: true });
    await page.waitForTimeout(150);
  }
  const opened = await page.locator('#settingsPanel').evaluate(element => {
    if (!element) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return !element.hidden && rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
  }).catch(() => false);
  if (!opened) {
    await page.evaluate(() => {
      if (typeof setSettingsPanelOpenPreserveMode === 'function') {
        setSettingsPanelOpenPreserveMode(true);
        return;
      }
      if (typeof setSettingsPanelOpen === 'function') {
        setSettingsPanelOpen(true);
        return;
      }
      const panel = document.querySelector('#settingsPanel');
      const toggle = document.querySelector('#settingsToggle');
      if (panel) panel.hidden = false;
      toggle?.classList.add('is-active');
      document.body.classList.add('is-task-settings-open');
    });
  }
  await page.locator('#settingsPanel').waitFor({ state: 'visible', timeout: 5000 });
}

async function getMainPage() {
  const mainUrl = new URL(devUrl);
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) {
    const pages = context.pages();
    for (const page of pages) {
      let urlMatches = false;
      try {
        const url = new URL(page.url());
        urlMatches = url.origin === mainUrl.origin && url.pathname === mainUrl.pathname;
      } catch {
        urlMatches = false;
      }
      if (!urlMatches) continue;
      const isMainSurface = await page.evaluate(() => {
        const body = document.body;
        return !!body
          && !body.classList.contains('is-preview-window')
          && !body.classList.contains('is-handoff-window')
          && !body.classList.contains('is-todo-window')
          && !body.classList.contains('is-performance-window')
          && !!document.querySelector('.watcher-window')
          && !!document.querySelector('#settingsToggle')
          && !!document.querySelector('#todoToggle');
      }).catch(() => false);
      if (isMainSurface) return page;
    }
    await delay(250);
  }
  throw new Error('等待 主窗口 超时。');
}

async function waitForPage(predicate, label, waitMs) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    const pages = context.pages();
    for (const page of pages) {
      if (predicate(page)) return page;
    }
    await delay(250);
  }
  throw new Error(`等待 ${label} 超时。`);
}

async function assertTauriPage(page, label) {
  const hasTauriInternals = await page.evaluate(() => Boolean(window.__TAURI_INTERNALS__));
  if (!hasTauriInternals) {
    throw new Error(`${label} 不是真实 Tauri WebViewWindow，检测不到 window.__TAURI_INTERNALS__。`);
  }
  return {
    窗口: label,
    地址: page.url(),
    标题: await page.title().catch(() => ''),
    Tauri运行时: true,
  };
}

async function visibleText(page) {
  return page.locator('body').innerText({ timeout: 10000 }).catch(() => '');
}

async function saveScreenshot(page, fileName) {
  const screenshotPath = path.join(outputDir, fileName);
  await page.screenshot({ path: screenshotPath, fullPage: true });
  result.截图.push(screenshotPath);
  return screenshotPath;
}

async function saveViewportScreenshot(page, fileName) {
  const screenshotPath = path.join(outputDir, fileName);
  await page.screenshot({ path: screenshotPath, fullPage: false });
  result.截图.push(screenshotPath);
  return screenshotPath;
}

async function nativeWindowBounds(page) {
  try {
    const session = await context.newCDPSession(page);
    const info = await session.send('Browser.getWindowForTarget');
    return info?.bounds || null;
  } catch {
    return null;
  }
}

async function setNativeWindowBounds(page, bounds) {
  try {
    const session = await context.newCDPSession(page);
    const info = await session.send('Browser.getWindowForTarget');
    await session.send('Browser.setWindowBounds', {
      windowId: info.windowId,
      bounds: { ...bounds, windowState: 'normal' },
    });
    await delay(500);
    return (await session.send('Browser.getWindowForTarget'))?.bounds || null;
  } catch {
    return null;
  }
}

async function captureFailureScreenshot() {
  if (!context) return;
  const page = context.pages()[0];
  if (!page) return;
  await saveScreenshot(page, 'failure.png');
}

function attachDiagnostics(nextContext) {
  for (const page of nextContext.pages()) attachPageDiagnostics(page);
  nextContext.on('page', page => attachPageDiagnostics(page));
}

function attachPageDiagnostics(page) {
  if (page.__agentWatcherDiagnosticsAttached) return;
  page.__agentWatcherDiagnosticsAttached = true;

  page.on('console', message => {
    if (!['warning', 'error'].includes(message.type())) return;
    pushLimited(result.诊断.浏览器日志, {
      类型: message.type() === 'warning' ? 'warn' : message.type(),
      地址: page.url(),
      文本: message.text(),
      位置: message.location(),
    });
  });

  page.on('pageerror', error => {
    pushLimited(result.诊断.页面错误, {
      地址: page.url(),
      错误: errorMessage(error),
    });
  });
}

function pushLimited(list, item, limit = 80) {
  list.push(item);
  if (list.length > limit) list.splice(0, list.length - limit);
}

async function collectWindows() {
  if (!context) return;
  result.窗口 = await Promise.all(context.pages().map(async page => ({
    地址: page.url(),
    标题: await page.title().catch(() => ''),
    Tauri运行时: await page.evaluate(() => Boolean(window.__TAURI_INTERNALS__)).catch(() => false),
  })));
}

async function runStep(name, fn) {
  const startedAt = Date.now();
  log(`步骤：${name}`);
  try {
    const detail = await fn();
    result.步骤.push({
      名称: name,
      通过: true,
      用时毫秒: Date.now() - startedAt,
      详情: detail,
    });
  } catch (error) {
    result.步骤.push({
      名称: name,
      通过: false,
      用时毫秒: Date.now() - startedAt,
      错误: errorMessage(error),
    });
    throw error;
  }
}

function writeResult() {
  const resultPath = path.join(outputDir, 'result.json');
  fs.writeFileSync(resultPath, JSON.stringify(result, null, 2), 'utf8');
  fs.mkdirSync(tmpRoot, { recursive: true });
  fs.writeFileSync(path.join(tmpRoot, 'latest-result.json'), JSON.stringify(result, null, 2), 'utf8');
}

async function waitForCdp(port, waitMs) {
  const deadline = Date.now() + waitMs;
  let lastError = '';
  while (Date.now() < deadline) {
    try {
      await fetchJson(`http://127.0.0.1:${port}/json/version`);
      log(`CDP 已就绪：127.0.0.1:${port}`);
      return;
    } catch (error) {
      lastError = errorMessage(error);
      await delay(500);
    }
  }
  throw new Error(`等待 WebView2 CDP 端口超时：${port}。最后错误：${lastError}`);
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} 返回 ${response.status}`);
  return response.json();
}

async function connectOverCdpWithRetry(chromium, port, waitMs) {
  const endpoint = `http://127.0.0.1:${port}`;
  const deadline = Date.now() + waitMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      return await chromium.connectOverCDP(endpoint);
    } catch (error) {
      lastError = error;
      await delay(500);
    }
  }
  throw lastError || new Error(`连接 WebView2 CDP 超时：${endpoint}`);
}

async function isHttpReady(url, waitMs) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      return response.ok;
    } catch {
      await delay(150);
    }
  }
  return false;
}

async function isDevServerReady(url, waitMs) {
  if (await isHttpReady(url, waitMs)) return true;
  try {
    const parsed = new URL(url);
    const port = Number(parsed.port || (parsed.protocol === 'https:' ? 443 : 80));
    return await waitForPort(parsed.hostname, port, 1000);
  } catch {
    return false;
  }
}

async function waitForPort(host, port, waitMs) {
  const deadline = Date.now() + waitMs;
  while (Date.now() < deadline) {
    if (await canListenToRemote(host, port)) return true;
    await delay(100);
  }
  return false;
}

function canListenToRemote(host, port) {
  return new Promise(resolve => {
    const socket = net.createConnection({ host, port });
    socket.setTimeout(500);
    socket.once('connect', () => {
      socket.destroy();
      resolve(true);
    });
    socket.once('timeout', () => {
      socket.destroy();
      resolve(false);
    });
    socket.once('error', () => resolve(false));
  });
}

async function findFreePort(startPort) {
  for (let port = startPort; port < startPort + 80; port += 1) {
    if (await canListen(port)) return port;
  }
  throw new Error(`没有找到可用 CDP 端口，起始端口：${startPort}`);
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

function spawnNpmDevUi(env) {
  const command = process.platform === 'win32' ? (process.env.ComSpec || 'cmd.exe') : 'npm';
  const commandArgs = process.platform === 'win32'
    ? ['/d', '/s', '/c', 'call npm run dev:ui']
    : ['run', 'dev:ui'];
  return spawn(command, commandArgs, {
    cwd: repoRoot,
    env,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function pipeChildToLog(child, logStream) {
  child.stdout?.pipe(logStream, { end: false });
  child.stderr?.pipe(logStream, { end: false });
  child.on('exit', code => {
    logStream.write(`\n[realtest] process ${child.pid} exited with code ${code ?? 'unknown'}\n`);
  });
  child.on('error', error => {
    logStream.write(`\n[realtest] process ${child.pid ?? 'unknown'} error: ${errorMessage(error)}\n`);
  });
}

function waitForProcessExit(child, label) {
  return new Promise((resolve, reject) => {
    child.once('error', error => reject(error));
    child.once('exit', code => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${label} exited with code ${code ?? 'unknown'}`));
      }
    });
  });
}

function cleanupStartedProcesses() {
  for (const child of startedProcesses.reverse()) {
    if (!child?.pid) continue;
    killProcessTree(child.pid);
  }
}

function detachStartedProcesses() {
  for (const child of startedProcesses) {
    try {
      child?.unref?.();
    } catch {}
  }
}

function killProcessTree(pid) {
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], {
      stdio: 'ignore',
      windowsHide: true,
    });
    return;
  }

  try {
    process.kill(pid, 'SIGTERM');
  } catch {}
}

function shutdown(code) {
  cleanupStartedProcesses();
  process.exit(code);
}

function log(message) {
  console.log(`[真实测试] ${message}`);
}

function fail(message) {
  console.error(`[真实测试] 失败：${message}`);
}

function errorMessage(error) {
  return error?.stack || error?.message || String(error);
}

function delay(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}
