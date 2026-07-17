import { describe, expect, test } from 'vitest';
import { browserLogs, editFile, page, waitForBuildStable } from '~utils';

// A dep-accepting boundary whose callback calls `import.meta.hot.invalidate()`
// must settle. Middleware-mode Vite guarantees this by ignoring a repeated
// invalidate from the same module until its next real update
// (`lastHMRInvalidationReceived`), and the old server-side FBM invalidate had
// the per-client `invalidateCalledModules` dedup. The client-side invalidate
// has no equivalent: with an import cycle giving the invalidator an executed
// importer, every re-walk lands on the same boundary, the boundary callback
// invalidates again, and the chain re-runs forever (~thousands of rounds per
// second, saturating the tab). This spec asserts the DESIRED behavior, so on
// the bug the loop test fails red: the settle poll hangs on the frozen page
// or the run/log counts explode.
//
// Sibling playground `hmr-client-invalidate` is the passing control for the
// non-cyclic invalidate flows (cold reload / warm hot-update).

const invalidateLogCount = () =>
  browserLogs.filter((l) => l.includes('invalidate') && l.includes('cycle-b'))
    .length;
const readCycleRuns = () =>
  page.evaluate(
    () => ((window as unknown as { __cycleRuns?: string[] }).__cycleRuns ?? []).length,
  );

describe('hmr-invalidate-loop', () => {
  test('should render initial content', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.cycle')).toBe('cycle-v1');
    await expect.poll(() => page.textContent('.plain')).toBe('plain-v1');
  });

  test('control: a self-accepting edit settles', async () => {
    await waitForBuildStable();
    editFile('plain.js', (code) => code.replace("'plain-v1'", "'plain-v2'"));
    await expect.poll(() => page.textContent('.plain')).toBe('plain-v2');
    await waitForBuildStable();
  });

  test('chained invalidate from a dep-accepting boundary settles', async () => {
    await waitForBuildStable();

    editFile('cycle-a.js', (code) =>
      code.replace("'cycle-v1'", "'cycle-v2'"),
    );
    await expect.poll(() => page.textContent('.cycle')).toBe('cycle-v2');

    // Settled means bounded work: the boundary re-ran cycle-a for the first
    // invalidate, and the repeated invalidate from cycle-b was ignored.
    // Under the loop these counts are in the thousands (or the poll above
    // already timed out on the saturated page).
    expect(await readCycleRuns()).toBeLessThanOrEqual(3);
    expect(invalidateLogCount()).toBeLessThanOrEqual(2);
  });
});
