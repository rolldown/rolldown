import { execa, ExecaError, type ResultPromise } from 'execa';
// @ts-expect-error `kill-port` does not have types
import killPortImpl from 'kill-port';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { chromium } from 'playwright';
import type { Browser, Page } from 'playwright';
import { afterAll, beforeAll, beforeEach } from 'vitest';
import { CONFIG } from './src/config';

let devServerProcess: ResultPromise<{}> | null = null;
let browser: Browser | null = null;
let page: Page | null = null;

async function killPort(port: number): Promise<void> {
  console.log(`[killPort] Killing any process on port ${port}...`);
  try {
    await killPortImpl(port);
  } catch (err) {
    if (
      err instanceof Error && err.message.includes('No process running')
    ) {
      console.log(`[killPort] No process running on port ${port}`);
    } else {
      throw err;
    }
  }
}

async function createTmpPlaygroundDir() {
  console.log(
    '[createTmpPlaygroundDir] Creating `tests/tmp-playground` playground directory...',
  );
  await nodeFs.promises.rm(CONFIG.paths.tmpPlaygroundDir, {
    recursive: true,
    force: true,
  });
  await nodeFs.promises.cp(
    CONFIG.paths.playgroundDir,
    CONFIG.paths.tmpPlaygroundDir,
    {
      recursive: true,
      dereference: false,
    },
  );
  console.log(
    '[createTmpPlaygroundDir] Created `tests/tmp-playground` playground directory.',
  );
}

/**
 * Files that are modified during tests and need to be reset on retry.
 * These files correspond to the source files edited by editFile() calls in hmr-full-bundle-mode.spec.ts.
 * If new files are edited in tests, add them here to ensure proper reset on retry.
 */
const TEST_FILES_TO_RESET = ['hmr.js', 'main.js'];

/**
 * Reset test files to their original state from the playground directory
 * This is needed for retry mechanism to work properly
 */
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
    throw new Error(
      `[resetTestFiles] Failed to reset ${errors.length} file(s): ${
        errors.map(e => e.filename).join(', ')
      }`,
    );
  }
}

async function waitForDevServerReady() {
  const maxAttempts = 30;
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch('http://localhost:3000');
      if (response.ok) {
        return;
      }
    } catch {}
    await new Promise(r => setTimeout(r, 50));
  }
  throw new Error('Server failed to start');
}

async function startDevServer() {
  console.log('[startDevServer] Starting dev server...');
  const subprocess = execa('pnpm serve', {
    cwd: CONFIG.paths.tmpFullBundleModeDir,
    shell: true,
    stdio: ['inherit', 'inherit', 'inherit'],
    env: {
      RUST_BACKTRACE: 'FULL',
      RD_LOG: process.env.RD_LOG || 'hmr=debug',
    },
  });

  // Handle errors separately without chaining
  subprocess.catch(err => {
    if (err instanceof ExecaError && err.signal === 'SIGTERM') {
      console.log(
        '[startDevServer] Dev server process terminated with SIGTERM.',
      );
    } else {
      throw err;
    }
  });
  await waitForDevServerReady();
  console.log('[startDevServer] Dev server started.');
  return { devServerProcess: subprocess };
}

/**
 * Before all tests: Start dev server and create browser page
 */
beforeAll(async () => {
  const createTmpPlaygroundDirPromise = createTmpPlaygroundDir();
  await killPort(3000);
  await createTmpPlaygroundDirPromise;
  ({ devServerProcess } = await startDevServer());

  console.log('[beforeAll] Launching browser...');
  browser = await chromium.launch({
    headless: !process.env.DEBUG_BROWSER,
  });

  // Create new page
  page = await browser.newPage();

  // Navigate to dev server
  await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });

  // Make page available to tests
  (global as any).__page = page;
});

beforeEach(async (ctx) => {
  const retryCount = ctx.task.result?.retryCount ?? 0;
  if (retryCount > 0) {
    await resetTestFiles();
    // Wait for file system watcher to detect and process the changes
    await new Promise(resolve => setTimeout(resolve, 1000 * 3));
    // Reload the page to ensure it reflects the reset file state
    // This is necessary because after a failed test, the page may show stale content
    if (page) {
      await page.reload({ waitUntil: 'networkidle' });
    }
  }
});

/**
 * After all tests: Clean up resources
 */
afterAll(async () => {
  // Close page
  if (page) {
    await page.close();
    page = null;
  }

  // Close browser
  if (browser) {
    await browser.close();
    browser = null;
  }

  // Kill dev server
  if (devServerProcess) {
    devServerProcess.kill('SIGTERM');
    devServerProcess = null;
  }
});
