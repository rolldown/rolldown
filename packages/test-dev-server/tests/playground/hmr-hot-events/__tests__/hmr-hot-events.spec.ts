import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's `hmr` "hot events" case: `import.meta.hot.on('vite:beforeUpdate' | 'vite:afterUpdate')`
// fire around a js-update. `listener.js` records them into `.events`; editing the self-accepting
// `target.js` triggers the update.

describe('hmr-hot-events', () => {
  test('renders initial state with no events yet', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('target-v1');
    await expect.poll(() => page.textContent('.events')).toBe('');
  });

  test('vite:beforeUpdate / vite:afterUpdate fire on a hot update', async () => {
    // No leading waitForBuildStable: the previous test already proved stability and
    // nothing has been edited since; every call costs its full stability window.
    await plantReloadMarker();

    editFile('target.js', (code) => code.replace("'target-v1'", "'target-v2'"));
    await expect.poll(() => page.textContent('.value')).toBe('target-v2');

    // Both built-in update events fired, in order, without a full reload.
    await expect.poll(() => page.textContent('.events')).toBe('before,after');
    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
