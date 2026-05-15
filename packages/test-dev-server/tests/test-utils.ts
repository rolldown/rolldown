import nodeFs from 'node:fs';
import { resolve } from 'node:path';
import { vi } from 'vitest';
import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { CONFIG } from './src/config.js';

const testDir = CONFIG.paths.tmpFullBundleModeDir;

/** Timeout (ms) for individual fetch requests to /_dev/status. */
const FETCH_TIMEOUT_MS = 5_000;

/**
 * Minimum time (ms) buildSeq must remain unchanged to consider the build stable.
 * Derived from the actual watcher debounce config with margin.
 */
const BUILD_STABLE_MS = (() => {
  const opts = getDevWatchOptionsForCi();
  return opts.debounceDuration + opts.debounceTickRate + 200;
})();

/**
 * Edit a file using Node.js fs module
 * Files are edited in the tmp directory, not the original source
 */
export async function editFile(
  filename: string,
  replacer: (content: string) => string,
): Promise<void> {
  await editFileInDir(testDir, filename, replacer);
}

/**
 * Edit a file in the lazy-shared-module tmp playground.
 */
export async function editLazySharedModuleFile(
  filename: string,
  replacer: (content: string) => string,
): Promise<void> {
  await editFileInDir(CONFIG.paths.tmpLazySharedModuleDir, filename, replacer);
}

async function editFileInDir(
  dir: string,
  filename: string,
  replacer: (content: string) => string,
): Promise<void> {
  const filePath = resolve(dir, filename);
  const content = nodeFs.readFileSync(filePath, 'utf-8');
  const newContent = replacer(content);
  if (content === newContent) {
    console.warn(`[editFile] No changes detected for ${filename}`);
    return;
  }
  nodeFs.writeFileSync(filePath, newContent, 'utf-8');
  console.log(`[editFile] Updated ${filename}`);
}

/**
 * Get the Playwright page from global context
 */
export function getPage() {
  const page = (global as any).__page;
  if (!page) {
    throw new Error('Playwright page not initialized. Check vitest-setup-browser.ts');
  }
  return page;
}

/**
 * Get the Playwright page for lazy compilation tests from global context
 */
export function getLazyPage() {
  const page = (global as any).__lazyPage;
  if (!page) {
    throw new Error('Lazy page not initialized. Check vitest-setup-browser.ts');
  }
  return page;
}

/**
 * Get the Playwright page for the lazy-shared-module playground.
 */
export function getLazySharedModulePage() {
  const page = (global as any).__lazySharedModulePage;
  if (!page) {
    throw new Error('lazy-shared-module page not initialized. Check vitest-setup-browser.ts');
  }
  return page;
}

/**
 * Get the Playwright page for the lazy-nested-dynamic-import regression test.
 */
export function getNestedLazyPage() {
  const page = (global as any).__nestedLazyPage;
  if (!page) {
    throw new Error(
      'lazy-nested-dynamic-import page not initialized. Check vitest-setup-browser.ts',
    );
  }
  return page;
}

interface DevStatus {
  hasStaleOutput: boolean;
  lastFullBuildFailed: boolean;
  buildSeq: number;
  connectedClients: number;
  moduleRegistrationSeq: number;
}

async function fetchDevStatus(port: number): Promise<DevStatus> {
  const res = await fetch(`http://localhost:${port}/_dev/status`, {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });
  if (!res.ok) {
    throw new Error(`/_dev/status responded with ${res.status}`);
  }
  return await res.json();
}

/** Poll until buildSeq increments past the given value (i.e., a new build completed). */
export async function waitForNextBuild(
  port: number,
  currentBuildSeq: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  return vi.waitFor(
    async () => {
      const status = await fetchDevStatus(port);
      if (status.buildSeq > currentBuildSeq) return status;
      throw new Error(`buildSeq still at ${status.buildSeq}, waiting for > ${currentBuildSeq}`);
    },
    { timeout: timeoutMs, interval: 50 },
  );
}

/**
 * Wait for buildSeq to stabilize (no changes for `stableMs`). This ensures the debounce window has closed.
 *
 * Uses BUILD_STABLE_MS derived from the actual watcher debounce config.
 */
export async function waitForBuildStable(
  port: number,
  stableMs = BUILD_STABLE_MS,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  const start = Date.now();
  let lastSeq = -1;
  let lastChangeTime = start;
  let lastError: unknown;
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(port);
      if (status.buildSeq !== lastSeq) {
        lastSeq = status.buildSeq;
        lastChangeTime = Date.now();
      } else if (Date.now() - lastChangeTime >= stableMs) {
        return status;
      }
    } catch (e) {
      lastError = e;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(
    `Build not stable within ${timeoutMs}ms` +
      (lastError ? `. Last fetch error: ${lastError}` : ''),
  );
}

/** Poll until moduleRegistrationSeq exceeds the given value (i.e., a new module registration happened). */
export async function waitForModuleRegistration(
  port: number,
  currentSeq: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  return vi.waitFor(
    async () => {
      const status = await fetchDevStatus(port);
      if (status.moduleRegistrationSeq > currentSeq) return status;
      throw new Error(
        `moduleRegistrationSeq still at ${status.moduleRegistrationSeq}, waiting for > ${currentSeq}`,
      );
    },
    { timeout: timeoutMs, interval: 50 },
  );
}

/** Get current module registration sequence number. */
export async function getModuleRegistrationSeq(port: number): Promise<number> {
  const status = await fetchDevStatus(port);
  return status.moduleRegistrationSeq;
}

/** Get current build sequence number. */
export async function getBuildSeq(port: number): Promise<number> {
  const status = await fetchDevStatus(port);
  return status.buildSeq;
}
