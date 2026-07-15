import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's #7561 case: `dep.js` sits behind a dynamic import that never runs, so it
// has an accept handler that never registered in this tab. Editing it must not disturb
// the page; editing the entry (which nothing accepts) still reloads as usual.

describe('hmr-not-loaded-dynamic-import', () => {
  test('counter works before any edit', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('button')).toBe('Counter 0');
    await page.click('button');
    await expect.poll(() => page.textContent('button')).toBe('Counter 1');
  });

  test('editing the entry reloads the page (control)', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    await plantReloadMarker();

    editFile('main.js', (code) => code + '\n');
    // The reload resets the counter to its initial markup.
    await expect.poll(() => readReloadMarker()).toBe(null);
    await expect.poll(() => page.textContent('button')).toBe('Counter 0');
    await page.click('button');
    await expect.poll(() => page.textContent('button')).toBe('Counter 1');
    await waitForBuildStable();
  });

  test('editing the never-loaded dynamic import does nothing', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('dep.js', (code) => code + '\n');
    // Wait out the rebuild, then prove the page was left alone: no reload, state intact.
    await waitForBuildStable();
    expect(await readReloadMarker()).toBe('alive');
    await expect.poll(() => page.textContent('button')).toBe('Counter 1');
  });
});
