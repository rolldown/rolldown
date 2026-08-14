import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's `hmr` dispose + `import.meta.hot.data` behavior: `dispose` runs before the
// module is replaced and stashes state on `data`, which persists across the update and is
// read back by the re-executed module.

describe('hmr-dispose-data', () => {
  test('first load has no persisted data', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('dispose-v1');
    await expect.poll(() => page.textContent('.prev')).toBe('none');
  });

  test('dispose stashes state on `data`, read back after the update', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    await plantReloadMarker();

    // The dispose callback saves the CURRENT value; the re-run reads it as `prev`.
    editFile('counter.js', (code) => code.replace("'dispose-v1'", "'dispose-v2'"));
    await expect.poll(() => page.textContent('.value')).toBe('dispose-v2');
    await expect.poll(() => page.textContent('.prev')).toBe('dispose-v1');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
