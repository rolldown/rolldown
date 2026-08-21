import { describe, expect, test } from 'vitest';
import { editFile, errorOverlayText, page, waitForBuildStable } from '~utils';

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

  test('unknown ids: an unknown id next to a known one is dropped', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('drop-some.txt', (code) => code.replace('drop-some-v1', 'drop-some-v2'));
    // The hook returned [missing.js, dep.js]: the unknown id is dropped
    // silently and dep.js still ships — no error, no reload.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('unknown ids: dropping every id ends the round as a noop', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('drop-all.txt', (code) => code.replace('drop-all-v1', 'drop-all-v2'));
    // The hook returned only [missing.js]: dropping it empties the set, which
    // is indistinguishable from an explicit suppression.
    await waitForBuildStable();

    expect(await readAcceptCount()).toBe(before);
    expect(await readMarker()).toBe('alive');
  });

  test('unmapped file: the chain runs with an empty set and the hook can claim modules', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('notes.txt', (code) => code.replace('notes-v1', 'notes-v2'));
    // notes.txt belongs to no module (plain buildStart watch), so the hook got
    // an empty default set (asserted inside the hook) and claimed dep.js. The
    // patch re-runs dep.js although dep's own code did not change —
    // hook-returned modules skip the unchanged-output suppression.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('decline: a real module edit goes through the default flow', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('dep.js', (code) => code.replace('dep-v1', 'dep-v2'));
    // The hook declined (returned undefined), so the edit flows exactly as if
    // no hook were registered: dep.js ships and the accept callback renders
    // the new value.
    await expect.poll(readAcceptCount).toBe(before + 1);
    await expect.poll(() => page.textContent('.value')).toBe('dep-v2');

    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('decline: a whitespace-only edit is still suppressed as unchanged output', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('dep.js', (code) => `${code}\n`);
    // Declining is not selecting: with identical rendered output the round
    // must end as a noop. Only hook-RETURNED modules skip this suppression.
    await waitForBuildStable();

    expect(await readAcceptCount()).toBe(before);
    expect(await readMarker()).toBe('alive');
  });

  test('mixed round: suppressing one file leaves another file\'s default flow alone', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    // Two files change back-to-back: suppress.txt (hook suppresses) and dep.js
    // (hook declines). Whether the watcher delivers them as one round or two,
    // the outcome must be the same: dep's edit ships exactly once, main never
    // ships (no reload), and the suppression eats nothing but its own file.
    editFile('suppress.txt', (code) => code.replace('suppress-v2', 'suppress-v3'));
    editFile('dep.js', (code) => code.replace('dep-v2', 'dep-v3'));

    await expect.poll(readAcceptCount).toBe(before + 1);
    await expect.poll(() => page.textContent('.value')).toBe('dep-v3');

    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
    expect(await readAcceptCount()).toBe(before + 1);
  });

  test('suppress cannot starve error recovery: a broken file still recovers', async () => {
    await waitForBuildStable();

    // Break dep.js: the round fails and the overlay appears.
    editFile('dep.js', (code) => code.replace("'dep-v3';", "'dep-v3"));
    await expect
      .poll(errorOverlayText, { timeout: 15_000 })
      .toMatch(/Unterminated|PARSE_ERROR|error/i);
    await waitForBuildStable();

    // Edit the suppressed file while the build is broken. The hook returns [],
    // but the engine folds the queued rescan of dep.js into the round BEFORE
    // the suppress decision, so the round re-fetches the still-broken file and
    // the overlay stays up.
    editFile('suppress.txt', (code) => code.replace('suppress-v3', 'suppress-v4'));
    await waitForBuildStable();
    expect(await errorOverlayText()).not.toBe('');

    // Restore dep.js to its exact pre-break bytes. Its rendered output equals
    // the last good build, so this recovers only because an errored build
    // skips the unchanged-output suppression — if the suppressed round above
    // had cleared the error state, this edit would be swallowed and the
    // overlay would never clear. Recovery may arrive as a patch or a full
    // reload — assert the outcome.
    editFile('dep.js', (code) => code.replace("'dep-v3", "'dep-v3';"));
    await expect.poll(errorOverlayText, { timeout: 15_000 }).toBe('');
    await expect.poll(() => page.textContent('.value')).toBe('dep-v3');
    await waitForBuildStable();
  });
});
