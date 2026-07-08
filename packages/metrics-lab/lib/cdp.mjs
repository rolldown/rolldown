// Minimal Chrome DevTools Protocol client over Node's built-in WebSocket (Node >= 22).
// No dependencies: launches headless Chrome/Edge with a throwaway profile, parses the
// "DevTools listening on ws://..." line, and exposes flat-protocol page sessions.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const CHROME_CANDIDATES = [
  process.env.CHROME_PATH,
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
  'C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe',
  process.env.LOCALAPPDATA
    && path.join(process.env.LOCALAPPDATA, 'Google', 'Chrome', 'Application', 'chrome.exe'),
  'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].filter(Boolean);

export function findChrome() {
  for (const candidate of CHROME_CANDIDATES) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error('No Chrome/Edge executable found. Set CHROME_PATH to a Chromium binary.');
}

export async function launchBrowser({ profileDir }) {
  if (typeof WebSocket === 'undefined') {
    throw new Error(`This harness needs Node >= 22 (global WebSocket); running ${process.version}.`);
  }
  const exe = findChrome();
  fs.rmSync(profileDir, { recursive: true, force: true });
  fs.mkdirSync(profileDir, { recursive: true });
  const child = spawn(exe, [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-component-update',
    '--disable-sync',
    '--disable-default-apps',
    '--mute-audio',
    '--window-size=1280,900',
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });

  const wsUrl = await new Promise((resolve, reject) => {
    let stderr = '';
    const timer = setTimeout(() => {
      cleanup();
      child.kill();
      reject(new Error(`Chrome did not announce a DevTools endpoint within 20s.\n${stderr}`));
    }, 20_000);
    const onData = (chunk) => {
      stderr += chunk;
      const match = stderr.match(/DevTools listening on (ws:\/\/\S+)/);
      if (match) {
        cleanup();
        resolve(match[1]);
      }
    };
    const onExit = (code) => {
      cleanup();
      reject(new Error(`Chrome exited before announcing DevTools endpoint (code ${code}).\n${stderr}`));
    };
    const cleanup = () => {
      clearTimeout(timer);
      child.stderr.off('data', onData);
      child.off('exit', onExit);
    };
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', onData);
    child.on('exit', onExit);
  });

  const cdp = await connect(wsUrl);
  return {
    cdp,
    async close() {
      try { cdp.close(); } catch { /* already closed */ }
      try { child.kill(); } catch { /* already gone */ }
      await new Promise((r) => setTimeout(r, 250));
      try { child.kill('SIGKILL'); } catch { /* already gone */ }
    },
  };
}

function connect(wsUrl) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let nextId = 1;
    const pending = new Map();
    const listeners = new Map(); // `${sessionId}:${method}` -> handlers
    ws.addEventListener('open', () => resolve(api));
    ws.addEventListener('error', () => reject(new Error(`WebSocket connection failed: ${wsUrl}`)));
    ws.addEventListener('message', (event) => {
      const msg = JSON.parse(event.data);
      if (msg.id === undefined) {
        const key = `${msg.sessionId ?? ''}:${msg.method}`;
        for (const handler of listeners.get(key) ?? []) handler(msg.params);
        return;
      }
      const entry = pending.get(msg.id);
      if (!entry) return;
      pending.delete(msg.id);
      if (msg.error) entry.reject(new Error(`${entry.method}: ${msg.error.message}`));
      else entry.resolve(msg.result);
    });
    const api = {
      send(method, params = {}, sessionId) {
        return new Promise((res, rej) => {
          const id = nextId++;
          pending.set(id, { resolve: res, reject: rej, method });
          ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
        });
      },
      on(method, handler, sessionId = '') {
        const key = `${sessionId}:${method}`;
        if (!listeners.has(key)) listeners.set(key, []);
        listeners.get(key).push(handler);
      },
      close() { ws.close(); },
    };
  });
}

/** Fresh tab configured for a measurement: cache off, optional throttle, optional init script. */
export async function openPage(cdp, { throttle, injectScript } = {}) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  const send = (method, params) => cdp.send(method, params, sessionId);
  await send('Page.enable');
  await send('Runtime.enable');
  await send('Network.enable');
  await send('Network.setCacheDisabled', { cacheDisabled: true });
  if (throttle) {
    await send('Network.emulateNetworkConditions', {
      offline: false,
      latency: throttle.latencyMs,
      downloadThroughput: throttle.downloadBps,
      uploadThroughput: throttle.uploadBps,
    });
    await send('Emulation.setCPUThrottlingRate', { rate: throttle.cpuRate });
  }
  if (injectScript) {
    await send('Page.addScriptToEvaluateOnNewDocument', { source: injectScript });
  }
  return {
    send,
    on: (method, handler) => cdp.on(method, handler, sessionId),
    navigate: (url) => send('Page.navigate', { url }),
    async evaluate(expression) {
      const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
      if (result.exceptionDetails) {
        const detail = result.exceptionDetails.exception?.description
          ?? result.exceptionDetails.text ?? 'unknown error';
        throw new Error(`page evaluate threw: ${detail}`);
      }
      return result.result.value;
    },
    close: () => cdp.send('Target.closeTarget', { targetId }),
  };
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
