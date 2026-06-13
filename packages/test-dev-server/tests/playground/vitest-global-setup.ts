import nodeFs from 'node:fs/promises';
import nodeOs from 'node:os';
import nodePath from 'node:path';
import type { BrowserServer } from 'playwright';
import { chromium } from 'playwright';
import type { TestProject } from 'vitest/node';

// Global setup for the browser e2e suite, ported from Vite's playground
// harness (see meta/design/dev-server-test-harness.md): launch one Chromium
// server for the whole run (each spec file opens its own page), and copy
// each playground used by the selected specs into playground-temp/ so tests
// edit a throwaway copy.

let browserServer: BrowserServer | undefined;
const PLAYGROUND_NAME_REGEX = /playground\/([\w-]+)\//;

const tempDir = nodePath.resolve(import.meta.dirname, '../playground-temp');

// DEBUG (CI flake #9727): dump the runtime environment and resolved config so
// CI logs explain matrix differences (e.g. node 20 vs 22/24, runner load).
// Grep for `[envdbg]`.
function dbgLogEnvAndConfig(project: TestProject): void {
  const c = project.config as unknown as Record<string, unknown>;
  const pick = {
    pool: c.pool,
    fileParallelism: c.fileParallelism,
    isolate: c.isolate,
    maxWorkers: c.maxWorkers,
    minWorkers: c.minWorkers,
    maxConcurrency: c.maxConcurrency,
    retry: c.retry,
    testTimeout: c.testTimeout,
    hookTimeout: c.hookTimeout,
    bail: c.bail,
    sequence: c.sequence,
  };
  console.log('[envdbg] ── e2e harness environment ──');
  console.log(`[envdbg] node=${process.version} platform=${process.platform} arch=${process.arch}`);
  console.log(
    `[envdbg] cpus=${nodeOs.cpus().length} totalmem=${Math.round(nodeOs.totalmem() / 2 ** 20)}MiB ` +
      `freemem=${Math.round(nodeOs.freemem() / 2 ** 20)}MiB loadavg=${nodeOs
        .loadavg()
        .map((n) => n.toFixed(2))
        .join(',')}`,
  );
  console.log(
    `[envdbg] env: CI=${process.env.CI} RD_LOG=${process.env.RD_LOG} ` +
      `RUST_BACKTRACE=${process.env.RUST_BACKTRACE} RD_TEST_RETRY=${process.env.RD_TEST_RETRY} ` +
      `RD_TEST_TIMEOUT=${process.env.RD_TEST_TIMEOUT}`,
  );
  try {
    console.log(`[envdbg] resolved config: ${JSON.stringify(pick)}`);
  } catch (err) {
    console.log(`[envdbg] config stringify failed: ${String(err)}`);
  }
}

export async function setup(project: TestProject): Promise<void> {
  dbgLogEnvAndConfig(project);
  browserServer = await chromium.launchServer({
    headless: !process.env.DEBUG_BROWSER,
    args: process.env.CI ? ['--no-sandbox', '--disable-setuid-sandbox'] : undefined,
  });
  project.provide('wsEndpoint', browserServer.wsEndpoint());

  const testFiles = project.vitest.state.getPaths();
  const playgroundNames = [
    ...new Set(
      testFiles
        .map((file) => file.replace(/\\/g, '/').match(PLAYGROUND_NAME_REGEX)?.[1])
        .filter((name): name is string => name != null),
    ),
  ];

  console.log(
    `[envdbg] spec files (${testFiles.length}): ` +
      JSON.stringify(testFiles.map((f) => f.replace(/\\/g, '/').split('/playground/')[1] ?? f)),
  );
  console.log(
    `[envdbg] playgrounds copied to playground-temp/: ${JSON.stringify(playgroundNames)}`,
  );

  await nodeFs.rm(tempDir, { recursive: true, force: true });
  await nodeFs.mkdir(tempDir, { recursive: true });
  await Promise.all(
    playgroundNames.map((name) =>
      nodeFs.cp(nodePath.resolve(import.meta.dirname, name), nodePath.resolve(tempDir, name), {
        recursive: true,
        filter: filterForPlaygroundCopy,
      }),
    ),
  );
}

export async function teardown(): Promise<void> {
  await browserServer?.close();
  await nodeFs.rm(tempDir, { recursive: true, force: true });
}

// Unlike Vite, node_modules is also skipped: playground-temp sits next to
// playground/, so bare imports still resolve to tests/node_modules by
// walking up — copying pnpm's symlink tree would be wasted work.
function filterForPlaygroundCopy(file: string): boolean {
  const normalized = file.replace(/\\/g, '/');
  return (
    !normalized.includes('__tests__') &&
    !/dist(?:\/|$)/.test(normalized) &&
    !normalized.includes('node_modules')
  );
}
