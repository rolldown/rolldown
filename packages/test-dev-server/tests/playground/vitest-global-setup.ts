import nodeFs from 'node:fs/promises';
import nodePath from 'node:path';
import type { BrowserServer } from 'playwright';
import { chromium } from 'playwright';
import type { TestProject } from 'vitest/node';

// Global setup for the browser e2e suite (the Vite playground harness
// transplanted — see meta/design/dev-server-test-harness.md): launch ONE
// Chromium server for the whole run (each spec file connects and opens its own
// page), and copy each selected playground into playground-temp/ so tests edit
// a throwaway copy. Playground selection is derived from the spec file paths —
// there is no central playground registry.

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

// Unlike Vite's copy filter, node_modules is also excluded: bare imports from
// playground-temp/<name>/ resolve by directory walk-up to tests/node_modules
// (playground-temp sits next to playground/, so the ancestry is identical),
// which makes copying pnpm's symlink forest wasted work and depth-sensitive.
function filterForPlaygroundCopy(file: string): boolean {
  const normalized = file.replace(/\\/g, '/');
  return (
    !normalized.includes('__tests__') &&
    !/dist(?:\/|$)/.test(normalized) &&
    !normalized.includes('node_modules')
  );
}
