// Per-file setup that runs in the SAME PROCESS as tests (setupFiles).
// Each test file gets its own servers, browser, and pages.
// Servers are killed in afterAll so the worker can exit cleanly.

import { execa } from 'execa';
// @ts-expect-error `kill-port` does not have types
import killPortImpl from 'kill-port';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { chromium } from 'playwright';
import type { Browser, Page } from 'playwright';
import { afterAll, beforeAll, beforeEach } from 'vitest';
import { CONFIG } from './src/config';

let browser: Browser | null = null;
let hmrPage: Page | null = null;
let lazyPage: Page | null = null;
let issue9312Page: Page | null = null;
let nestedLazyPage: Page | null = null;

async function killPort(port: number): Promise<void> {
  try {
    await killPortImpl(port);
  } catch (err) {
    if (err instanceof Error && err.message.includes('No process running')) {
      // Nothing to kill
    } else {
      throw err;
    }
  }
}

const TEST_FILES_TO_RESET = ['hmr.js', 'main.js'];

async function resetTestFiles() {
  for (const filename of TEST_FILES_TO_RESET) {
    const srcPath = nodePath.join(CONFIG.paths.hmrFullBundleModeDir, filename);
    const destPath = nodePath.join(CONFIG.paths.tmpFullBundleModeDir, filename);
    const originalContent = await nodeFs.promises.readFile(srcPath, 'utf-8');
    await nodeFs.promises.writeFile(destPath, originalContent, 'utf-8');
  }
}

async function waitForDevServerReady(port: number) {
  const maxAttempts = 60;
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`http://localhost:${port}`);
      if (response.ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`Server failed to start on port ${port}`);
}

function startDevServer(cwd: string) {
  const subprocess = execa('pnpm serve', {
    cwd,
    shell: true,
    stdio: ['inherit', 'inherit', 'inherit'],
    env: {
      RUST_BACKTRACE: 'FULL',
      RD_LOG: process.env.RD_LOG || 'hmr=debug',
    },
  });
  // Suppress expected termination errors
  subprocess.catch(() => {});
  return subprocess;
}

beforeAll(async () => {
  // Kill any existing processes on our ports
  await Promise.all([
    killPort(CONFIG.ports.hmrFullBundleMode),
    killPort(CONFIG.ports.lazyCompilation),
    killPort(CONFIG.ports.lazyIssue9312),
    killPort(CONFIG.ports.lazyNestedDynamicImport),
  ]);

  // Always recreate tmp playground from source to pick up any fixture changes.
  await nodeFs.promises.rm(CONFIG.paths.tmpPlaygroundDir, { recursive: true, force: true });
  await nodeFs.promises.cp(CONFIG.paths.playgroundDir, CONFIG.paths.tmpPlaygroundDir, {
    recursive: true,
    dereference: false,
  });

  // Reset HMR test files to original state
  await resetTestFiles();

  // Start dev servers (ports configured in each playground's dev.config.mjs).
  // lazy-issue-9312 is self-contained (not a pnpm workspace member), so it runs
  // the CLI directly via `node <abs path>` instead of `pnpm serve`.
  startDevServer(CONFIG.paths.tmpFullBundleModeDir);
  startDevServer(CONFIG.paths.tmpLazyCompilationDir);
  startDevServer(CONFIG.paths.tmpLazyIssue9312Dir);
  startDevServer(CONFIG.paths.tmpLazyNestedDynamicImportDir);

  // Wait for servers to be ready
  await Promise.all([
    waitForDevServerReady(CONFIG.ports.hmrFullBundleMode),
    waitForDevServerReady(CONFIG.ports.lazyCompilation),
    waitForDevServerReady(CONFIG.ports.lazyIssue9312),
    waitForDevServerReady(CONFIG.ports.lazyNestedDynamicImport),
  ]);

  // Launch browser and create pages
  browser = await chromium.launch({ headless: !process.env.DEBUG_BROWSER });

  hmrPage = await browser.newPage();
  lazyPage = await browser.newPage();
  issue9312Page = await browser.newPage();
  nestedLazyPage = await browser.newPage();

  // Only navigate the HMR page here. The lazy page is NOT navigated in setup
  // to avoid warming the lazy-compilation server (main.js triggers a dynamic
  // import after 1s, which would pre-compile the lazy module before the test).
  // The issue-9312 page is also navigated by the test itself; it relies on a
  // user click to fire the dynamic imports, so pre-navigating is harmless,
  // but keeping symmetry with the lazy page makes intent clearer.
  await hmrPage.goto(`http://localhost:${CONFIG.ports.hmrFullBundleMode}`, {
    waitUntil: 'networkidle',
  });

  (global as any).__page = hmrPage;
  (global as any).__lazyPage = lazyPage;
  (global as any).__issue9312Page = issue9312Page;
  (global as any).__nestedLazyPage = nestedLazyPage;
});

beforeEach(async (ctx) => {
  const retryCount = ctx.task.result?.retryCount ?? 0;
  if (retryCount > 0) {
    await resetTestFiles();
    await new Promise((resolve) => setTimeout(resolve, 1000 * 3));
    const hmrPage = (global as any).__page;
    if (hmrPage) {
      await hmrPage.reload({ waitUntil: 'networkidle' });
    }
  }
});

afterAll(async () => {
  // Close pages and browser
  if (hmrPage) {
    await hmrPage.close().catch(() => {});
    hmrPage = null;
  }
  if (lazyPage) {
    await lazyPage.close().catch(() => {});
    lazyPage = null;
  }
  if (issue9312Page) {
    await issue9312Page.close().catch(() => {});
    issue9312Page = null;
  }
  if (nestedLazyPage) {
    await nestedLazyPage.close().catch(() => {});
    nestedLazyPage = null;
  }
  if (browser) {
    await browser.close().catch(() => {});
    browser = null;
  }

  // Kill dev servers so the worker process can exit cleanly.
  // Use killPort (reliable cross-platform) as the primary mechanism.
  await Promise.all([
    killPort(CONFIG.ports.hmrFullBundleMode),
    killPort(CONFIG.ports.lazyCompilation),
    killPort(CONFIG.ports.lazyIssue9312),
    killPort(CONFIG.ports.lazyNestedDynamicImport),
  ]).catch(() => {});
});
