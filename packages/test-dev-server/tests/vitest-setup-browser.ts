// Per-file setup - runs in the SAME PROCESS as tests.
// This is where we set up browser/pages because globalThis is shared with tests.
// Directory creation and server startup are in globalSetup (runs once).

import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { chromium } from 'playwright';
import type { Browser, Page } from 'playwright';
import { afterAll, beforeAll, beforeEach } from 'vitest';
import { CONFIG } from './src/config';

let browser: Browser | null = null;
let hmrPage: Page | null = null;
let lazyPage: Page | null = null;

const TEST_FILES_TO_RESET = ['hmr.js', 'main.js'];

async function resetTestFiles() {
  const errors: Array<{ filename: string; error: unknown }> = [];
  for (const filename of TEST_FILES_TO_RESET) {
    const srcPath = nodePath.join(CONFIG.paths.hmrFullBundleModeDir, filename);
    const destPath = nodePath.join(CONFIG.paths.tmpFullBundleModeDir, filename);
    try {
      const originalContent = await nodeFs.promises.readFile(srcPath, 'utf-8');
      await nodeFs.promises.writeFile(destPath, originalContent, 'utf-8');
      console.log(`[resetTestFiles] Reset ${filename} to original state`);
    } catch (err) {
      console.error(`[resetTestFiles] Failed to reset ${filename}:`, err);
      errors.push({ filename, error: err });
    }
  }
  if (errors.length > 0) {
    throw new Error(`[resetTestFiles] Failed to reset ${errors.length} file(s)`);
  }
}

async function waitForDevServerReady(port: number) {
  const maxAttempts = 60; // More attempts since server is already starting
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`http://localhost:${port}`);
      if (response.ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`Server not ready on port ${port}`);
}

beforeAll(async () => {
  console.log('[setupFiles] Waiting for dev servers...');
  // Dev servers are already started by globalSetup, just wait for them
  await Promise.all([
    waitForDevServerReady(CONFIG.ports.hmrFullBundleMode),
    waitForDevServerReady(CONFIG.ports.lazyCompilation),
  ]);

  console.log('[setupFiles] Launching browser...');
  browser = await chromium.launch({ headless: !process.env.DEBUG_BROWSER });

  hmrPage = await browser.newPage();
  lazyPage = await browser.newPage();

  await Promise.all([
    hmrPage.goto(`http://localhost:${CONFIG.ports.hmrFullBundleMode}`, {
      waitUntil: 'networkidle',
    }),
    lazyPage.goto(`http://localhost:${CONFIG.ports.lazyCompilation}`, { waitUntil: 'networkidle' }),
  ]);

  // Set pages on globalThis - THIS IS NOW ACCESSIBLE IN TESTS!
  (globalThis as any).__page = hmrPage;
  (globalThis as any).__lazyPage = lazyPage;
  console.log('[setupFiles] Browser and pages ready.');
});

beforeEach(async (ctx) => {
  const retryCount = ctx.task.result?.retryCount ?? 0;
  if (retryCount > 0) {
    await resetTestFiles();
    await new Promise((resolve) => setTimeout(resolve, 1000 * 3));
    const hmrPage = (globalThis as any).__page;
    if (hmrPage) {
      await hmrPage.reload({ waitUntil: 'networkidle' });
    }
  }
});

afterAll(async () => {
  // Close pages
  if (hmrPage) {
    await hmrPage.close();
    hmrPage = null;
  }
  if (lazyPage) {
    await lazyPage.close();
    lazyPage = null;
  }
  // Close browser
  if (browser) {
    await browser.close();
    browser = null;
  }
  // NOTE: Don't kill dev servers here - they need to stay running for other test files
  // globalTeardown will kill them after all tests complete
});
