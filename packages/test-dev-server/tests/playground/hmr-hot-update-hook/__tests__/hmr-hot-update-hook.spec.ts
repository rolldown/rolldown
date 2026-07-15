import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// End-to-end check of the experimental `hotUpdate` plugin hook (see dev.config.mjs):
// the hook maps `config.txt` edits to dep.js (replace semantics) and swallows
// `suppress.txt` edits (suppress semantics). A full reload wipes `window.__marker`,
// so a surviving marker proves the update was applied (or skipped) in place.

const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);
const readAcceptCount = () =>
  page.evaluate(() => (window as unknown as { __acceptCount?: number }).__acceptCount ?? -1);

describe('hmr-hot-update-hook', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('dep-v1');
    expect(await readAcceptCount()).toBe(0);
  });

  test('replace: editing config.txt hot-updates dep.js via the hook', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('config.txt', (code) => code.replace('config-v1', 'config-v2'));
    // The hook returned [dep.js], so the patch re-runs dep.js and main's accept
    // callback for './dep.js' fires exactly once.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('suppress: editing suppress.txt produces no update', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('suppress.txt', (code) => code.replace('suppress-v1', 'suppress-v2'));
    // The hook returned [], so this build round must end in a Noop: same accept
    // count, no reload. waitForBuildStable synchronizes on the server's build
    // state instead of sleeping.
    await waitForBuildStable();

    expect(await readAcceptCount()).toBe(before);
    expect(await readMarker()).toBe('alive');
  });
});
