import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// End-to-end check that the `hotUpdate` chain hands each plugin the set as
// edited by the plugins before it (see dev.config.mjs). In both scenarios the
// final set is `[dep.js]`, so the patch re-runs dep.js and main's accept
// callback fires — but each scenario only works if the second plugin saw the
// first plugin's edit (contract violations throw inside the hooks, so the
// accept-count poll below would time out).

const readAcceptCount = () =>
  page.evaluate(() => (window as unknown as { __acceptCount?: number }).__acceptCount ?? -1);

// SKIPPED: the `hotUpdate` hook is off by default (`hotUpdate` dev
// option, see rolldown/rolldown#10714). This playground runs on Vite's bundled
// dev, which does not pass that option yet; un-skip once Vite can enable it
// (vitejs/vite#22956).
describe.skip('hmr-hot-update-hook-chain', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('dep-v1');
    expect(await readAcceptCount()).toBe(0);
  });

  test('re-add: the second plugin can restore modules after an empty return', async () => {
    await waitForBuildStable();
    await plantReloadMarker();
    const before = await readAcceptCount();

    editFile('readd.txt', (code) => code.replace('readd-v1', 'readd-v2'));
    // First plugin suppressed, second re-added dep.js — the update must ship.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readReloadMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('keep: a decline keeps the earlier replacement, not the default', async () => {
    await waitForBuildStable();
    await plantReloadMarker();
    const before = await readAcceptCount();

    editFile('keep.txt', (code) => code.replace('keep-v1', 'keep-v2'));
    // First plugin replaced with dep.js, second declined — dep.js still ships.
    // If the decline had reset the set to the default (main.js, no boundary),
    // this would full-reload instead.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
