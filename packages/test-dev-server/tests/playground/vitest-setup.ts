// Per-file setup for the browser e2e suite, ported from Vite's
// `playground/vitestSetup.ts` (see meta/design/dev-server-test-harness.md).
// Each spec file finds its playground from its own path, connects to the
// shared Chromium server, opens one page, and starts an in-process dev
// server — or runs a custom `__tests__/serve.ts` if the playground has one.
// Teardown just closes the page and the server.

import {
  createDevServer,
  type DevServerHandle,
  loadDevConfig,
  type Logger,
} from '@rolldown/test-dev-server';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import type { Browser, Page } from 'playwright';
import { chromium } from 'playwright';
import { beforeAll, beforeEach, expect, inject, onTestFailed } from 'vitest';

export type { DevServerHandle } from '@rolldown/test-dev-server';

const PLAYGROUND_NAME_REGEX = /playground\/([\w-]+)\//;

// Repo locations resolved from this file (tests/playground/vitest-setup.ts).
const playgroundDir = import.meta.dirname;
const playgroundTempDir = nodePath.resolve(playgroundDir, '../playground-temp');

// --- Exported bindings (the `~utils` surface) -------------------------------

/** Path to the current spec file. */
export let testPath: string = '';
/** Playground name, derived from the spec path (e.g. `hmr-full-bundle-mode`). */
export let testName: string = '';
/**
 * Absolute playground root for the current spec — the throwaway copy under
 * `playground-temp/<name>/`. `editFile`/`readFile`/etc. are relative to it.
 */
export let testDir: string = '';

export let browser: Browser = undefined!;
export let page: Page = undefined!;

/** Resolved URL of the running dev server (OS-assigned port included). */
export let serverUrl: string = '';

export const browserLogs: string[] = [];
export const browserErrors: Error[] = [];
export const serverLogs: string[] = [];

let serverHandle: DevServerHandle | undefined;

/**
 * Context passed to a playground's optional `__tests__/serve.ts`.
 * `createServer` loads the playground's `dev.config.mjs` and starts the
 * server. A custom serve usually just calls it and returns the handle
 * without navigating, so the spec controls the first request.
 */
export interface ServeContext {
  testName: string;
  testDir: string;
  page: Page;
  createServer: () => Promise<DevServerHandle>;
}

/** In-memory logger collecting server output into `logs` (Vite's `customLogger`). */
export function createInMemoryLogger(logs: string[]): Logger {
  const format = (args: unknown[]) => args.map((a) => String(a)).join(' ');
  return {
    info: (...args) => logs.push(format(args)),
    warn: (...args) => logs.push(format(args)),
    error: (...args) => logs.push(format(args)),
    debug: (...args) => logs.push(format(args)),
  };
}

/** Load `<testDir>/dev.config.mjs` and start an in-process dev server for it. */
async function createServerForTest(testDir: string): Promise<DevServerHandle> {
  const config = await loadDevConfig(testDir);
  // Relative paths in the config resolve against `cwd`, which in-process
  // would be the tests dir — pin it to the playground copy instead. This
  // also lets the server find the playground's `index.html`.
  const build = { ...config.build, cwd: testDir };
  return createDevServer({ ...config, build }, { logger: createInMemoryLogger(serverLogs) });
}

// A custom serve lives next to the spec in the source `__tests__/` dir. That
// dir is not copied to playground-temp, so resolve it from the spec's path,
// not from `testDir`.
function findCustomServe(specDir: string): string | undefined {
  for (const ext of ['ts', 'js', 'mjs']) {
    const candidate = nodePath.join(specDir, `serve.${ext}`);
    if (nodeFs.existsSync(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

// eslint-disable-next-line no-empty-pattern
beforeAll(async ({}, suite) => {
  testPath = suite.file.filepath;
  testName = testPath.replace(/\\/g, '/').match(PLAYGROUND_NAME_REGEX)?.[1] ?? '';
  // Tests run against the throwaway copy, not the source playground.
  testDir = nodePath.resolve(playgroundTempDir, testName);

  const wsEndpoint = inject('wsEndpoint');
  if (!wsEndpoint) {
    throw new Error('wsEndpoint not found (is vitest-global-setup.ts registered?)');
  }

  browser = await chromium.connect(wsEndpoint);
  page = await browser.newPage();

  try {
    page.on('console', (msg) => {
      browserLogs.push(msg.text());
    });
    page.on('pageerror', (error) => {
      browserErrors.push(error);
    });

    const customServe = findCustomServe(nodePath.dirname(testPath));
    if (customServe) {
      // The playground manages its own server and navigation. The lazy
      // playground uses this to keep the server untouched until the spec
      // navigates.
      const mod = await import(customServe);
      const serve: (ctx: ServeContext) => Promise<DevServerHandle> =
        mod.serve ?? mod.default?.serve;
      if (typeof serve !== 'function') {
        throw new Error(`${customServe} must export a \`serve\` function`);
      }
      const ctx: ServeContext = {
        testName,
        testDir,
        page,
        createServer: () => createServerForTest(testDir),
      };
      const handle = await serve(ctx);
      serverHandle = handle;
      serverUrl = handle.url;
    } else {
      // Default path: start the server and navigate to it.
      serverHandle = await createServerForTest(testDir);
      serverUrl = serverHandle.url;
      await page.goto(serverUrl);
    }
  } catch (e) {
    // Close the page so a setup failure shows up here, not as a confusing
    // `page.click` timeout later.
    await page.close().catch(() => {});
    await serverHandle?.close().catch(() => {});
    throw e;
  }

  return async () => {
    browserLogs.length = 0;
    browserErrors.length = 0;
    serverLogs.length = 0;
    await page?.close().catch(() => {});
    await serverHandle?.close().catch(() => {});
    serverHandle = undefined;
    await browser?.close().catch(() => {});
  };
});

// --- DEBUG instrumentation (CI flake #9727) ---------------------------------
// These specs mutate a shared on-disk fixture (playground-temp/<name>/) via
// anchored string replacement, with no per-test/per-retry reset. The theory is
// that one flaky first-attempt failure leaves the fixture off-baseline, after
// which retries and sibling specs can never recover (editFile silently
// no-ops). To confirm, snapshot the whole fixture tree at the start of every
// attempt and at failure time. Grep the CI log for `[fixtdbg]`.

const FIXTURE_TEXT_EXT = new Set(['.js', '.mjs', '.cjs', '.ts', '.json', '.txt', '.html', '.css']);

function dbgSnapshotFixtures(): string[] {
  const lines: string[] = [];
  const walk = (dir: string, rel: string): void => {
    let entries: nodeFs.Dirent[];
    try {
      entries = nodeFs.readdirSync(dir, { withFileTypes: true });
    } catch (err) {
      lines.push(`[fixtdbg] <readdir failed for ${rel || '.'}: ${String(err)}>`);
      return;
    }
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
      const abs = nodePath.join(dir, entry.name);
      const relPath = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (entry.name === 'node_modules' || entry.name === 'dist' || entry.name === '.git')
          continue;
        walk(abs, relPath);
      } else if (FIXTURE_TEXT_EXT.has(nodePath.extname(entry.name))) {
        try {
          const c = nodeFs.readFileSync(abs, 'utf-8');
          const shown = c.length > 400 ? `${c.slice(0, 400)}…(+${c.length - 400})` : c;
          lines.push(`[fixtdbg]   ${relPath} len=${c.length} = ${JSON.stringify(shown)}`);
        } catch (err) {
          lines.push(`[fixtdbg]   ${relPath} <read failed: ${String(err)}>`);
        }
      }
    }
  };
  walk(testDir, '');
  return lines;
}

const dbgAttempts = new Map<string, number>();

beforeEach((context) => {
  const name = context.task?.name ?? expect.getState().currentTestName ?? '<unknown>';
  const key = context.task?.id ?? `${testName}::${name}`;
  const attempt = (dbgAttempts.get(key) ?? 0) + 1;
  dbgAttempts.set(key, attempt);
  const retryCount = context.task?.result?.retryCount;
  console.log(
    `[fixtdbg] ── BEFORE [${testName}] "${name}" attempt#${attempt} (vitest.retryCount=${retryCount ?? 'n/a'}) serverUrl=${serverUrl}`,
  );
  for (const line of dbgSnapshotFixtures()) console.log(line);

  onTestFailed((ctx) => {
    console.error(`[fixtdbg] ── FAILED [${testName}] "${name}" attempt#${attempt}`);
    for (const err of ctx.task?.result?.errors ?? []) {
      console.error(`[fixtdbg]   error: ${err?.name}: ${err?.message}`);
      if (err?.stack) {
        console.error(
          `[fixtdbg]   stack: ${String(err.stack).split('\n').slice(0, 6).join(' | ')}`,
        );
      }
    }
    console.error('[fixtdbg]   fixtures at failure:');
    for (const line of dbgSnapshotFixtures()) console.error(line);
    console.error(`[fixtdbg]   serverLogs tail: ${JSON.stringify(serverLogs.slice(-30))}`);
    console.error(`[fixtdbg]   browserLogs tail: ${JSON.stringify(browserLogs.slice(-30))}`);
    console.error(
      `[fixtdbg]   browserErrors: ${JSON.stringify(browserErrors.map((e) => e.message))}`,
    );
  });
});

declare module 'vitest' {
  export interface ProvidedContext {
    wsEndpoint: string;
  }
}
