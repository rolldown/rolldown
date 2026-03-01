// Global setup runs ONCE before all test files in a SEPARATE PROCESS.
// IMPORTANT: globalThis here is NOT shared with test files!
// Browser/page setup must be in setupFiles which shares globalThis with tests.

import { execa, ExecaError } from 'execa';
// @ts-expect-error `kill-port` does not have types
import killPortImpl from 'kill-port';
import nodeFs from 'node:fs';
import nodePath from 'node:path';
import { CONFIG } from './src/config';
import { removeDirSync } from './src/utils';

async function killPort(port: number): Promise<void> {
  console.log(`[killPort] Killing any process on port ${port}...`);
  try {
    await killPortImpl(port);
  } catch (err) {
    if (err instanceof Error && err.message.includes('No process running')) {
      console.log(`[killPort] No process running on port ${port}`);
    } else {
      throw err;
    }
  }
}

function createTmpPlaygroundDir(srcDir: string, destDir: string) {
  console.log(
    `[createTmpPlaygroundDir] Creating tmp directory for ${nodePath.basename(srcDir)}...`,
  );
  removeDirSync(destDir);
  nodeFs.mkdirSync(nodePath.dirname(destDir), { recursive: true });
  nodeFs.cpSync(srcDir, destDir, { recursive: true, dereference: false });
  console.log(`[createTmpPlaygroundDir] Created tmp directory for ${nodePath.basename(srcDir)}.`);
}

async function waitForDevServerReady(port: number) {
  const maxAttempts = 30;
  for (let i = 0; i < maxAttempts; i++) {
    try {
      const response = await fetch(`http://localhost:${port}`);
      if (response.ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`Server failed to start on port ${port}`);
}

async function startDevServer(cwd: string, port: number) {
  console.log(`[startDevServer] Starting dev server on port ${port}...`);
  const subprocess = execa('pnpm serve', {
    cwd,
    shell: true,
    stdio: ['inherit', 'inherit', 'inherit'],
    env: { RUST_BACKTRACE: 'FULL', RD_LOG: process.env.RD_LOG || 'hmr=debug' },
  });
  // Silently ignore termination errors - the server will be killed by globalTeardown
  subprocess.catch((err) => {
    if (
      err instanceof ExecaError &&
      (err.signal === 'SIGTERM' ||
        err.signal === 'SIGKILL' ||
        err.isTerminated ||
        err.exitCode === 1)
    ) {
      // Expected: server was killed during cleanup
    } else {
      console.error(`[startDevServer] Dev server on port ${port} failed:`, err);
    }
  });
  await waitForDevServerReady(port);
  console.log(`[startDevServer] Dev server started on port ${port}.`);
}

/**
 * Global setup: Runs ONCE before all test files
 * Creates directories and starts dev servers.
 * NOTE: Browser/page setup is in setupFiles, not here, because
 * globalThis in this process is NOT shared with test files.
 */
export async function setup() {
  // Kill any existing processes on our ports
  await Promise.all([
    killPort(CONFIG.ports.hmrFullBundleMode),
    killPort(CONFIG.ports.lazyCompilation),
  ]);

  // Create tmp directories (uses Windows-aware removeDirSync)
  createTmpPlaygroundDir(CONFIG.paths.hmrFullBundleModeDir, CONFIG.paths.tmpFullBundleModeDir);
  createTmpPlaygroundDir(CONFIG.paths.lazyCompilationDir, CONFIG.paths.tmpLazyCompilationDir);

  // Start dev servers (they'll keep running as separate processes)
  await Promise.all([
    startDevServer(CONFIG.paths.tmpFullBundleModeDir, CONFIG.ports.hmrFullBundleMode),
    startDevServer(CONFIG.paths.tmpLazyCompilationDir, CONFIG.ports.lazyCompilation),
  ]);

  console.log('[globalSetup] Dev servers started. Browser setup will happen in setupFiles.');
}

/**
 * Global teardown: Runs ONCE after all test files
 * Kills dev servers by port (we don't have process handles here)
 */
export async function teardown() {
  console.log('[globalTeardown] Killing dev servers...');
  await Promise.all([
    killPort(CONFIG.ports.hmrFullBundleMode),
    killPort(CONFIG.ports.lazyCompilation),
  ]);
  console.log('[globalTeardown] Cleanup complete.');
}
