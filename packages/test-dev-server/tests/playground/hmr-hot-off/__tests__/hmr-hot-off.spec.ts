import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's `hot.off` coverage: a listener removed with `off` must not fire, while a
// listener that stays registered does. Vite tests `off` with plugin custom events; FBM has
// no plugin channel, so the built-in `vite:beforeUpdate` event exercises the same machinery.

describe('hmr-hot-off', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('off-v1');
  });

  test('off-removed listener stays silent; kept listener fires', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    await plantReloadMarker();

    editFile('target.js', (code) => code.replace("'off-v1'", "'off-v2'"));
    await expect.poll(() => page.textContent('.value')).toBe('off-v2');

    const calls = await page.evaluate(() => ({
      removed: (window as any).__removedCalls ?? 0,
      kept: (window as any).__keptCalls ?? 0,
    }));
    expect(calls.kept).toBeGreaterThanOrEqual(1);
    expect(calls.removed).toBe(0);

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
