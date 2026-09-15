import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Boundaries are decided by the BROWSER from accepts that actually EXECUTED,
// not from the compiler's static scan. A statically-visible accept in a dead
// branch is not a boundary (clean full reload, never a stale page); a
// conditional accept whose condition was true at execution time is.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () => page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-runtime-acceptance', () => {
  test('should render initial content', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.dead')).toBe('dead-v1');
    await expect.poll(() => page.textContent('.cond')).toBe('cond-v1');
  });

  // Run the hot case first: the dead-branch case below full-reloads the page.
  test('an accept that executed (behind `if (true)`) hot-updates', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('cond.js', (code) => code.replace("'cond-v1'", "'cond-v2'"));
    await expect.poll(() => page.textContent('.cond')).toBe('cond-v2');

    // No reload happened: the marker survived.
    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('an accept that never executed (dead branch) full-reloads onto fresh content', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('dead.js', (code) => code.replace("'dead-v1'", "'dead-v2'"));

    // The client's walk finds no executed boundary -> clean full page reload.
    // The reloaded page runs a fresh bundle with the NEW content — never a
    // silently stale page.
    await expect.poll(() => page.textContent('.dead')).toBe('dead-v2');
    await expect.poll(readMarker).toBe(null);

    await waitForBuildStable();
  });
});
