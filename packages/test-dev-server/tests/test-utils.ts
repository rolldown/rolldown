import nodeFs from 'node:fs';
import { resolve } from 'node:path';
import { CONFIG } from './src/config.js';

const testDir = CONFIG.paths.tmpFullBundleModeDir;

/**
 * Edit a file using Node.js fs module
 * Files are edited in the tmp directory, not the original source
 */
export async function editFile(
  filename: string,
  replacer: (content: string) => string,
): Promise<void> {
  const filePath = resolve(testDir, filename);
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
    throw new Error('Playwright page not initialized. Check vitest-setup-playwright.ts');
  }
  return page;
}

interface DevStatus {
  hasStaleOutput: boolean;
  lastFullBuildFailed: boolean;
  buildSeq: number;
  connectedClients: number;
  registeredClients: number;
}

async function fetchDevStatus(port: number): Promise<DevStatus> {
  const res = await fetch(`http://localhost:${port}/_dev/status`);
  return res.json();
}

/** Poll until buildSeq increments past the given value (i.e., a new build completed). */
export async function waitForNextBuild(
  port: number,
  currentBuildSeq: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(port);
      if (status.buildSeq > currentBuildSeq) return status;
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`No new build within ${timeoutMs}ms (stuck at buildSeq=${currentBuildSeq})`);
}

/** Poll until pipeline is idle (not stale). */
export async function waitForDevIdle(port: number, timeoutMs = 30_000): Promise<DevStatus> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(port);
      if (!status.hasStaleOutput) return status;
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`Dev server not idle within ${timeoutMs}ms`);
}

/** Wait for buildSeq to stabilize (no changes for `stableMs`). This ensures the debounce window has closed. */
export async function waitForBuildStable(
  port: number,
  stableMs = 800,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  const start = Date.now();
  let lastSeq = -1;
  let lastChangeTime = start;
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(port);
      if (status.buildSeq !== lastSeq) {
        lastSeq = status.buildSeq;
        lastChangeTime = Date.now();
      } else if (Date.now() - lastChangeTime >= stableMs) {
        return status;
      }
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(`Build not stable within ${timeoutMs}ms`);
}

/** Poll until registeredClients exceeds the given count (i.e., a new module registration happened). */
export async function waitForModuleRegistration(
  port: number,
  currentCount: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(port);
      if (status.registeredClients > currentCount) return status;
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error(
    `Module registration not reached (stuck at ${currentCount}) within ${timeoutMs}ms`,
  );
}

/** Get current build sequence number. */
export async function getBuildSeq(port: number): Promise<number> {
  const status = await fetchDevStatus(port);
  return status.buildSeq;
}

/** Get current registered client count. */
export async function getRegisteredClients(port: number): Promise<number> {
  const status = await fetchDevStatus(port);
  return status.registeredClients;
}
