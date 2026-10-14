// Browser access for field runs. The lab launches a throwaway headless Chrome;
// a deployed site behind a login needs the user's own browser instead, because
// copied cookies rarely survive device-bound sessions and bot checks. So field
// mode ATTACHES to a running Chrome over CDP and works in the browser context
// (profile) of a tab that already shows the site.
//
// Endpoint, in order: --cdp-url ws://..., --cdp-port N, or the DevToolsActivePort
// file that Chrome writes when remote debugging is on (chrome://inspect/#remote-debugging).

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { connect } from '../cdp.mjs';

function activePortFiles() {
  const home = os.homedir();
  const dirs = [
    process.env.CHROME_USER_DATA_DIR,
    path.join(home, 'Library', 'Application Support', 'Google', 'Chrome'),
    path.join(home, 'Library', 'Application Support', 'Google', 'Chrome Canary'),
    path.join(home, '.config', 'google-chrome'),
    path.join(home, '.config', 'chromium'),
    process.env.LOCALAPPDATA &&
      path.join(process.env.LOCALAPPDATA, 'Google', 'Chrome', 'User Data'),
  ].filter(Boolean);
  return dirs.map((dir) => path.join(dir, 'DevToolsActivePort'));
}

export async function resolveEndpoint({ cdpUrl, cdpPort } = {}) {
  if (cdpUrl) return cdpUrl;
  if (cdpPort) {
    const res = await fetch(`http://127.0.0.1:${cdpPort}/json/version`);
    return (await res.json()).webSocketDebuggerUrl;
  }
  for (const file of activePortFiles()) {
    if (!fs.existsSync(file)) continue;
    const [port, wsPath] = fs.readFileSync(file, 'utf8').trim().split('\n');
    if (port && wsPath) return `ws://127.0.0.1:${port}${wsPath}`;
  }
  throw new Error(
    'no running Chrome with remote debugging found. Turn it on at chrome://inspect/#remote-debugging,\n' +
      'or start Chrome with --remote-debugging-port=<n> and pass --cdp-port <n>.',
  );
}

export async function attachBrowser(options) {
  const wsUrl = await resolveEndpoint(options);
  const cdp = await connect(wsUrl);
  return { cdp, close: async () => cdp.close() };
}

/** The browser context of an open tab on `origin`: that profile holds the login. */
export async function contextForOrigin(cdp, origin) {
  const { targetInfos } = await cdp.send('Target.getTargets');
  const tab = targetInfos.find((t) => t.type === 'page' && t.url.startsWith(origin));
  return tab ? { browserContextId: tab.browserContextId, found: true } : { found: false };
}

// --- cookies -------------------------------------------------------------------------
// Variant cookies change the whole profile, not only the measuring tab, so the
// original values are saved first and restored at the end (also on Ctrl-C).
// Values stay in memory only; nothing here is printed.

const domainMatches = (cookieDomain, host) => {
  const d = cookieDomain.replace(/^\./, '');
  return host === d || host.endsWith(`.${d}`);
};

export async function snapshotCookies(cdp, { browserContextId, origin, names }) {
  const host = new URL(origin).hostname;
  const { cookies } = await cdp.send(
    'Storage.getCookies',
    browserContextId ? { browserContextId } : {},
  );
  const saved = new Map();
  for (const name of names) {
    saved.set(name, cookies.find((c) => c.name === name && domainMatches(c.domain, host)) ?? null);
  }
  return saved;
}

const cookieParam = (c) => ({
  name: c.name,
  value: c.value,
  domain: c.domain,
  path: c.path,
  secure: c.secure,
  httpOnly: c.httpOnly,
  ...(c.sameSite ? { sameSite: c.sameSite } : {}),
  ...(c.session ? {} : { expires: c.expires }),
});

/**
 * Put back each saved cookie. A name the user did not have is deleted with an
 * expired cookie in the same scope the variant used: `url` for host-only
 * cookies, `domain` when the config set one.
 */
export async function restoreCookies(cdp, { browserContextId, origin, saved, configured = [] }) {
  const cookies = [];
  for (const [name, original] of saved) {
    if (original) {
      cookies.push(cookieParam(original));
      continue;
    }
    for (const c of configured.filter((x) => x.name === name)) {
      cookies.push({
        name,
        value: '',
        path: c.path ?? '/',
        ...(c.domain ? { domain: c.domain } : { url: origin }),
        expires: 1,
      });
    }
  }
  if (cookies.length) {
    await cdp.send('Storage.setCookies', {
      cookies,
      ...(browserContextId ? { browserContextId } : {}),
    });
  }
}

// --- measuring tab ---------------------------------------------------------------------

/**
 * A new window in the chosen profile, configured like the lab tab: cache off,
 * service worker bypassed, blocked URLs, the variant's cookies, init scripts.
 * A variant cookie reuses the domain/path of the user's existing cookie of the
 * same name, so the profile never ends up with two cookies of that name.
 * `bodies` raises the response buffers so script bodies stay readable for
 * build matching (diagnostic runs only).
 */
export async function openFieldPage(
  cdp,
  {
    browserContextId,
    origin,
    cookies = [],
    saved = new Map(),
    block = [],
    scripts = [],
    bodies = false,
    throttle = null,
  },
) {
  const { targetId } = await cdp.send('Target.createTarget', {
    url: 'about:blank',
    newWindow: true,
    ...(browserContextId ? { browserContextId } : {}),
  });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  // A covered or background window is "hidden", and Chrome stops reporting LCP
  // for hidden pages. Bring the window to the front; runs still check visibility.
  await cdp.send('Target.activateTarget', { targetId }).catch(() => {});
  const send = (method, params) => cdp.send(method, params, sessionId);
  await send('Page.enable');
  await send('Runtime.enable');
  await send(
    'Network.enable',
    bodies ? { maxTotalBufferSize: 400_000_000, maxResourceBufferSize: 64_000_000 } : {},
  );
  await send('Network.setCacheDisabled', { cacheDisabled: true });
  await send('Network.setBypassServiceWorker', { bypass: true }).catch(() => {});
  if (block.length) {
    await send('Network.setBlockedURLs', {
      urls: block.map((p) => (p.includes('*') ? p : `*${p}*`)),
    });
  }
  if (throttle) {
    await send('Network.emulateNetworkConditions', {
      offline: false,
      latency: throttle.latencyMs,
      downloadThroughput: throttle.downloadBps,
      uploadThroughput: throttle.uploadBps,
    });
    await send('Emulation.setCPUThrottlingRate', { rate: throttle.cpuRate });
  }
  const secure = origin.startsWith('https:');
  for (const cookie of cookies) {
    const existing = saved.get(cookie.name);
    await send('Network.setCookie', {
      url: origin,
      path: '/',
      secure,
      ...(existing
        ? { domain: existing.domain, path: existing.path, secure: existing.secure }
        : {}),
      ...cookie,
    });
  }
  for (const source of scripts) {
    await send('Page.addScriptToEvaluateOnNewDocument', { source });
  }
  return {
    send,
    on: (method, handler) => cdp.on(method, handler, sessionId),
    navigate: (url) => send('Page.navigate', { url }),
    async evaluate(expression) {
      const result = await send('Runtime.evaluate', {
        expression,
        returnByValue: true,
        awaitPromise: true,
      });
      if (result.exceptionDetails) {
        throw new Error(
          `page evaluate threw: ${result.exceptionDetails.exception?.description ?? result.exceptionDetails.text}`,
        );
      }
      return result.result.value;
    },
    close: () => cdp.send('Target.closeTarget', { targetId }),
  };
}
