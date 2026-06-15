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
]);

const flowDisplay = {
  smoke: '冒烟流程',
  'agent-task': 'AgentTask 面板流程',
  performance: '性能面板流程',
  handoff: '接续面板流程',
  'opencode-session': 'OpenCode 会话卡片流程',
  'opencode-handoff-context': 'OpenCode 接续上下文流程',
  'settings-performance': '设置切换性能流程',
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
const selectedFlows = normalizeFlows(args.flows);

const result = {
  工具: 'AgentWatcher Tauri 真实交互测试',
  说明: '通过 Playwright 连接 Windows WebView2 CDP，验证真实 Tauri 桌面壳，不使用浏览器 mock。',
  开始时间: runStartedAt.toISOString(),
  仓库: repoRoot,
  开发地址: devUrl,
  输出目录: outputDir,
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
  browser = await playwright.chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`);
  context = browser.contexts()[0];
  attachDiagnostics(context);

  await runStep('切换主窗口界面语言为中文', () => ensureChineseUi());

  for (const flow of selectedFlows) {
    if (flow === 'smoke') {
      await runStep('确认主窗口是真实 Tauri 运行时', () => assertMainWindow());
      await runStep('点击打开性能诊断窗口', () => runPerformanceFlow());
      await runStep('确认接续面板 WebViewWindow 可被识别', () => runHandoffFlow());
      await runStep('点击切换到 AgentTask 面板', () => runAgentTaskFlow());
    } else if (flow === 'agent-task') {
      await runStep('点击切换到 AgentTask 面板', () => runAgentTaskFlow());
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

function normalizeFlows(rawFlows) {
  if (rawFlows.length === 0) return ['smoke'];

  const flows = rawFlows.map(flow => {
    const normalized = aliasToFlow.get(String(flow).trim().toLowerCase());
    if (!normalized) {
      throw new Error(`未知流程：${flow}。可用流程：冒烟、任务面板、性能面板、接续面板、opencode-session、opencode-handoff-context、settings-performance。`);
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
  };

  if (await isDevServerReady(devUrl, 5000) && fs.existsSync(debugExePath)) {
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

  log('未检测到可复用的 Vite，启动 npm run dev。');
  const logPath = path.join(outputDir, 'tauri-dev.log');
  const logStream = fs.createWriteStream(logPath, { flags: 'a' });
  const devCommand = process.platform === 'win32' ? (process.env.ComSpec || 'cmd.exe') : 'npm';
  const devArgs = process.platform === 'win32'
    ? ['/d', '/s', '/c', 'call npm run dev']
    : ['run', 'dev'];
  const dev = spawn(devCommand, devArgs, {
    cwd: repoRoot,
    env,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  dev.stdout.pipe(logStream);
  dev.stderr.pipe(logStream);
  startedProcesses.push(dev);
  result.开发日志 = logPath;
  return '启动 npm run dev';
}

async function assertMainWindow() {
  const page = await getMainPage();
  const info = await assertTauriPage(page, '主窗口');
  const screenshot = await saveScreenshot(page, 'main-tauri.png');
  return { ...info, 截图: screenshot };
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
  await page.waitForFunction(() => document.body.innerText.includes('全部 Agent 会话'), null, { timeout: 15000 });

  const after = await page.evaluate(() => ({
    htmlLang: document.documentElement.lang,
    bodyLang: document.body.dataset.lang || '',
    主标题: document.querySelector('[data-i18n="allSessions"]')?.textContent || document.body.innerText.slice(0, 80),
  }));

  return { 操作: '通过设置面板切换到中文界面', 切换前: before, 切换后: after };
}

async function runAgentTaskFlow() {
  const page = await getMainPage();
  await assertTauriPage(page, '主窗口');
  const toggle = page.locator('#todoToggle');
  await toggle.waitFor({ state: 'visible', timeout: 10000 });
  const before = await toggle.getAttribute('aria-label');

  await toggle.click();
  await page.waitForFunction(() => document.body.classList.contains('is-agent-task-mode'), null, { timeout: 10000 });

  const after = await toggle.getAttribute('aria-label');
  const bodyClass = await page.evaluate(() => document.body.className);
  const screenshot = await saveScreenshot(page, 'agent-task-after-click.png');

  return {
    点击目标: '#todoToggle',
    点击前: before,
    点击后: after,
    bodyClass,
    截图: screenshot,
  };
}

async function runPerformanceFlow() {
  const main = await getMainPage();
  await assertTauriPage(main, '主窗口');
  const toggle = main.locator('#performanceToggle');
  await toggle.waitFor({ state: 'visible', timeout: 10000 });

  const pressedBefore = await toggle.getAttribute('aria-pressed');
  if (pressedBefore === 'true') {
    await toggle.click();
    await main.waitForFunction(() => document.querySelector('#performanceToggle')?.getAttribute('aria-pressed') === 'false', null, { timeout: 10000 });
  }

  const pagesBefore = new Set(context.pages());
  await toggle.click();

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
  const handoff = await waitForPage(
    page => page.url().includes('?handoff=1'),
    '接续面板窗口',
    20000,
  );
  await handoff.waitForLoadState('domcontentloaded');
  const info = await assertTauriPage(handoff, '接续面板窗口');
  const text = await visibleText(handoff);
  const screenshot = await saveScreenshot(handoff, 'handoff-window.png');

  return {
    ...info,
    可见文本片段: text.slice(0, 180),
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
  const beforeToast = await page.locator('#toast').textContent().catch(() => '');

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
  const visible = await page.locator('#settingsPanel').evaluate(element => {
    if (!element) return false;
    const rect = element.getBoundingClientRect();
    const style = window.getComputedStyle(element);
    return !element.hidden && rect.width > 0 && rect.height > 0 && style.display !== 'none' && style.visibility !== 'hidden';
  }).catch(() => false);
  if (!visible) await page.locator('#settingsToggle').click();
  await page.locator('#settingsPanel').waitFor({ state: 'visible', timeout: 5000 });
}

async function getMainPage() {
  const mainUrl = new URL(devUrl);
  return waitForPage(page => {
    try {
      const url = new URL(page.url());
      return url.origin === mainUrl.origin && url.pathname === mainUrl.pathname && !url.search;
    } catch {
      return false;
    }
  }, '主窗口', 20000);
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
