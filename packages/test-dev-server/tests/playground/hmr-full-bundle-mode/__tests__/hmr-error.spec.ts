import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

const SLOT = '/* @syntax-error-slot */';
const BREAK = "const broken = '";

// Covers the design principles in meta/design/dev-engine.md for an HMR
// failure: a syntax error makes the HMR update fail and the overlay shows
// (Design Principle 2). Refreshing the page then triggers a full rebuild —
// the one exception in Design Principle 3 where page access starts a build,
// to get past a possibly broken HMR path. Here the source is still broken,
// so that build fails too; after it, refreshing triggers nothing (Design
// Principle 1). Fixing the file recovers (Design Principle 3).
describe('hmr-full-bundle-mode: HMR-stage failure', () => {
  test('page refresh after an Hmr-stage failure triggers a full rebuild', async () => {
    await waitForBuildStable();

    // Break the file with a syntax error; the HMR update fails.
    editFile('hmr-error/module.js', (code) => code.replace(SLOT, BREAK));

    const overlay = page.locator('#rolldown-error-overlay');
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    // The page still runs the last good bundle.
    expect(await page.textContent('.hmr-error')).toBe('hmr-error: ok');

    const { buildSeq: seqWhileBroken, lastBuildErrored } = await waitForBuildStable();
    expect(lastBuildErrored).toBe(true);

    // The exception in Design Principle 3: reload after an HMR failure
    // triggers a full rebuild. It fails again (the file is still broken),
    // but a new build ran — buildSeq moved. Compare rebuild-error.spec.ts,
    // where a reload builds nothing.
    await page.reload();
    const afterReload = await waitForBuildStable();
    expect(afterReload.buildSeq).toBeGreaterThan(seqWhileBroken);
    expect(afterReload.lastBuildErrored).toBe(true);
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);

    // The failure is now a full-build failure, not an HMR one — so another
    // reload triggers nothing (Design Principle 1).
    await page.reload();
    await expect.poll(() => overlay.count(), { timeout: 15_000 }).toBe(1);
    const afterSecondReload = await waitForBuildStable();
    expect(afterSecondReload.buildSeq).toBe(afterReload.buildSeq);

    // Design Principle 3: fix the file — the build succeeds, the server
    // reloads the page, and the overlay clears.
    editFile('hmr-error/module.js', (code) => code.replace(BREAK, SLOT));
    await expect
      .poll(() => page.textContent('.hmr-error'), { timeout: 15_000 })
      .toBe('hmr-error: ok');
    await expect.poll(() => overlay.count()).toBe(0);
    await waitForBuildStable();
  });
});
