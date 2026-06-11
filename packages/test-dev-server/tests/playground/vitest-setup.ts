// Per-file setup for the browser e2e suite, ported from Vite's
// `playground/vitestSetup.ts` (see meta/design/dev-server-test-harness.md).
// Each spec file derives its own playground from its path, connects to the
// shared Chromium server, opens one page, and starts an in-process dev server
// (default path) or runs a custom `__tests__/serve.ts` (lazy cold-start path).
// No subprocess, no port registry, no killPort: teardown closes the page and
// calls the server's own `close()`.

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
import { beforeAll, inject } from 'vitest';

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
 * Context handed to a playground's optional `__tests__/serve.ts`. `createServer`
 * loads the playground's `dev.config.mjs` and starts an in-process server
 * (logger + cwd already wired); a custom serve typically just calls it and
 * returns the handle WITHOUT navigating, so the spec controls the first
 * request (cold-start lazy semantics).
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
  // Relative `input` paths in the config resolve against `cwd`; in-process the
  // worker's cwd is the tests dir, so pin it to the playground copy (the
  // subprocess model got this for free via the child's cwd). This also lets
  // `createDevServerPlugin` find the playground's `index.html`.
  const build = { ...config.build, cwd: testDir };
  return createDevServer({ ...config, build }, { logger: createInMemoryLogger(serverLogs) });
}

// A custom serve lives next to the spec, in the SOURCE `__tests__/` dir — that
// dir is filtered out of the temp copy (like Vite), so it must be resolved
// from the spec's own path, not from `testDir`.
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
      // Escape hatch: the playground manages its own server (and whether/when
      // to navigate). Used by the lazy playgrounds to keep the server cold.
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
    // Close the page so a setup failure (e.g. a build crash) surfaces here
    // instead of as a downstream `page.click` timeout that hides the cause.
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

declare module 'vitest' {
  export interface ProvidedContext {
    wsEndpoint: string;
  }
}
