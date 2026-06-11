import { createDevServer, loadDevConfig } from '@rolldown/test-dev-server';
import { describe, expect, test } from 'vitest';
import {
  getBuildSeq as getBuildSeqByUrl,
  waitForBuildStable as waitForBuildStableByUrl,
} from '../../../src/dev-status';
import {
  createInMemoryLogger,
  type DevServerHandle,
  editFile,
  page,
  readFile,
  serverLogs,
  testDir,
} from '~utils';

const SLOT = '/* @syntax-error-slot */';
const BREAK = "const broken = '";

// Covers the design principles in meta/design/dev-engine.md for a failing
// FIRST build:
// - Design Principle 1 (Conservative rebuilds): refreshing never retries
//   the build
// - Design Principle 2 (Errors are emitted on every build): the error
//   reaches the browser and survives a refresh
// - Design Principle 3 (File changes are the only recovery trigger): fixing
//   the file recovers — the broken file is watched even though it never
//   parsed
describe('hmr-full-bundle-mode: initial build failure', () => {
  test('error on first load, access never retries, a file change recovers', async () => {
    // The default server already built the working sources, so a first-build
    // failure can't happen there. Break the module first, then start a second
    // server on the broken sources.
    editFile('initial-error/module.js', (code) => code.replace(SLOT, BREAK));

    let server: DevServerHandle | undefined;
    try {
      const config = await loadDevConfig(testDir);
      // A build error does not fail server startup; the server still serves.
      server = await createDevServer(
        { ...config, build: { ...config.build, cwd: testDir } },
        { logger: createInMemoryLogger(serverLogs) },
      );

      // Design Principle 2: there is no output yet, so the spinner page is
      // served and the saved build error shows up as an overlay on top.
      await page.goto(server.url);
      const overlay = page.locator('#rolldown-error-overlay');
      await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);

      // Design Principle 1: refreshing never retries the build — without
      // new input the same error would just happen again.
      const seqFailed = await getBuildSeqByUrl(server.url);
      await page.reload();
      await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
      const status = await waitForBuildStableByUrl(server.url);
      expect(status.buildSeq).toBe(seqFailed);
      expect(status.lastBuildErrored).toBe(true);

      // Design Principle 3: fixing the file triggers a new build, and the
      // server reloads the page onto the working app.
      editFile('initial-error/module.js', (code) => code.replace(BREAK, SLOT));
      await expect
        .poll(() => page.textContent('.initial-error'), { timeout: 15_000 })
        .toBe('initial-error: ok');
      await expect.poll(() => overlay.count()).toBe(0);
    } finally {
      await server?.close();
      // Restore the fixture even if an assertion above failed.
      if (readFile('initial-error/module.js').includes(BREAK)) {
        editFile('initial-error/module.js', (code) => code.replace(BREAK, SLOT));
      }
    }
  });
});
