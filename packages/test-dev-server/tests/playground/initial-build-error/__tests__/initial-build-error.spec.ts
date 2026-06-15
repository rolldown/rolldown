import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

const SLOT = '/* @syntax-error-slot */';
const BREAK = "const broken = '";

// Covers meta/design/dev-engine.md for a failing FIRST build. This playground
// SHIPS BROKEN (`module.js` ends with an unterminated string), so the harness's
// first build already failed and `page` is on the spinner + error overlay — the
// spec creates no server of its own.
// - Principle 1 (Conservative rebuilds): refreshing never retries the build.
// - Principle 2 (Errors are emitted on every build): the error reaches the
//   browser and survives a refresh.
// - Principle 3 (File changes are the only recovery trigger): fixing the file
//   recovers — the broken file is watched even though it never parsed.
describe('initial-build-error', () => {
  test('error on first load, access never retries, a file change recovers', async () => {
    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);

    // Principle 1: refreshing must not retry the build — without new input the
    // same error would just happen again.
    const { buildSeq: seqFailed, lastBuildErrored } = await waitForBuildStable();
    expect(lastBuildErrored).toBe(true);
    await page.reload();
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    const afterReload = await waitForBuildStable();
    expect(afterReload.buildSeq).toBe(seqFailed);
    expect(afterReload.lastBuildErrored).toBe(true);

    // Principle 3: fixing the file triggers a new build and the server reloads
    // the page onto the working app.
    editFile('module.js', (code) => code.replace(BREAK, SLOT));
    await expect
      .poll(() => page.textContent('.app'), { timeout: 15_000 })
      .toBe('initial-build-error: ok');
    await expect.poll(() => overlay.count()).toBe(0);
  });
});
