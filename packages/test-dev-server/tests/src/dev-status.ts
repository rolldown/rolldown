import { getDevWatchOptionsForCi } from '@rolldown/test-dev-server';
import { vi } from 'vitest';

// Helpers around the dev server's test-only `/_dev/status` endpoint, keyed by
// server URL (hook-free so both the node fixtures suite and the browser
// harness can import them). The browser harness re-exports thin wrappers that
// default the URL to the current spec's server — see playground/test-utils.ts.

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

export interface DevStatus {
  hasStaleOutput: boolean;
  lastBuildErrored: boolean;
  buildSeq: number;
  connectedClients: number;
  moduleRegistrationSeq: number;
}

async function fetchDevStatus(serverUrl: string): Promise<DevStatus> {
  const res = await fetch(new URL('/_dev/status', serverUrl), {
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
  });
  if (!res.ok) {
    throw new Error(`/_dev/status responded with ${res.status}`);
  }
  return await res.json();
}

/** Poll until buildSeq increments past the given value (i.e., a new build completed). */
export async function waitForNextBuild(
  serverUrl: string,
  currentBuildSeq: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  return vi.waitFor(
    async () => {
      const status = await fetchDevStatus(serverUrl);
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
  serverUrl: string,
  stableMs = BUILD_STABLE_MS,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  const start = Date.now();
  let lastSeq = -1;
  let lastChangeTime = start;
  let lastError: unknown;
  while (Date.now() - start < timeoutMs) {
    try {
      const status = await fetchDevStatus(serverUrl);
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
  serverUrl: string,
  currentSeq: number,
  timeoutMs = 30_000,
): Promise<DevStatus> {
  return vi.waitFor(
    async () => {
      const status = await fetchDevStatus(serverUrl);
      if (status.moduleRegistrationSeq > currentSeq) return status;
      throw new Error(
        `moduleRegistrationSeq still at ${status.moduleRegistrationSeq}, waiting for > ${currentSeq}`,
      );
    },
    { timeout: timeoutMs, interval: 50 },
  );
}

/** Get current module registration sequence number. */
export async function getModuleRegistrationSeq(serverUrl: string): Promise<number> {
  const status = await fetchDevStatus(serverUrl);
  return status.moduleRegistrationSeq;
}

/** Get current build sequence number. */
export async function getBuildSeq(serverUrl: string): Promise<number> {
  const status = await fetchDevStatus(serverUrl);
  return status.buildSeq;
}
