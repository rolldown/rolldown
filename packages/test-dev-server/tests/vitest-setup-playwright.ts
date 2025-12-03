import '@testing-library/jest-dom/vitest';
import { execa, ExecaError, type ResultPromise } from 'execa';
import killPortImpl from 'kill-port';
import nodeFs from 'node:fs';
import { chromium } from 'playwright';
import type { Browser, Page } from 'playwright';
import { afterAll, beforeAll } from 'vitest';
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
    '[createTmpPlaygroundDir] Creating `tests/playground-tmp` playground directory...',
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
    '[createTmpPlaygroundDir] Created `tests/playground-tmp` playground directory.',
  );
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
    await new Promise(r => setTimeout(r, 200));
  }
  throw new Error('Server failed to start');
}

async function startDevServer() {
  console.log('[startDevServer] Starting dev server...');
  const subprocess = execa('pnpm serve', {
    cwd: CONFIG.paths.tmpFullBundleModeDir, // CHANGED: Use tmp directory instead of original
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
    headless: !process.env.DEBUG_BROWSER, // Can be controlled via env var
  });

  // Create new page
  page = await browser.newPage();

  // Navigate to dev server
  await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });

  // Make page available to tests
  (global as any).__page = page;
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
