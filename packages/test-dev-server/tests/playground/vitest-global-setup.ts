import nodeFs from 'node:fs/promises';
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

export async function setup(project: TestProject): Promise<void> {
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
