import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Ports Vite's `invalidation-circular-deps` cases. Both sub-graphs are circles
// (child imports parent, parent imports child) whose child accepts and then
// immediately invalidates:
// - circular-invalidate: the parent ALSO accept-then-invalidates, so nothing can handle
//   the edit; the client must settle on a clean reload with fresh content instead of
//   looping the invalidation forever.
// - invalidate-handled-in-circle: the parent self-accepts for real, so the invalidation
//   bubbles one level, the parent re-runs, and fresh content lands without a reload.

describe('hmr-invalidate-circular', () => {
  test('renders both circles', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.invalidation-circular-deps')).toBe('child');
    await expect.poll(() => page.textContent('.invalidation-circular-deps-handled')).toBe('child');
  });

  test('unhandled invalidate in a circle settles on fresh content, no infinite HMR', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    editFile('circular-invalidate/child.js', (code) =>
      code.replace("'child'", "'child updated'"));
    await expect
      .poll(() => page.textContent('.invalidation-circular-deps'), { timeout: 15_000 })
      .toBe('child updated');
    await waitForBuildStable();
  });

  test('invalidate handled one level up in the circle hot-updates', async () => {
    editFile('invalidate-handled-in-circle/child.js', (code) =>
      code.replace("'child'", "'child updated'"));
    await expect
      .poll(() => page.textContent('.invalidation-circular-deps-handled'), { timeout: 15_000 })
      .toBe('child updated');
    await waitForBuildStable();
  });
});
