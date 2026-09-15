import { describe, expect, test } from 'vitest';
import { browserLogs, editFile, page, waitForBuildStable } from '~utils';

// `import.meta.hot.invalidate()` is fully client-side: the client re-walks
// from the invalidator's importers and applies locally when its factory map
// covers the new update set, else it does a clean full reload (never a stale
// page). No websocket round trip is involved — the server never hears about
// the invalidate.
//
// Factory coverage comes from the ship map: an edit of `inner` ships
// only inner's factory (the server's update-superset walk stops at inner's static
// self-accept), so on a COLD session the invalidate re-walk finds no factory
// for `outer` and falls back to a clean reload. Once outer's factory has been
// delivered (by editing outer once), the same invalidate hot-updates.

const plantMarker = () => page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);
const readOuterRuns = () =>
  page.evaluate(() => (window as unknown as { __outerRuns?: string[] }).__outerRuns ?? []);

describe('hmr-client-invalidate', () => {
  test('should render initial content', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.outer')).toBe('inner-v1');
    await expect.poll(() => page.textContent('.lone')).toBe('lone-v1');
  });

  test('invalidate without factory coverage degrades to a clean reload (cold session)', async () => {
    await waitForBuildStable();
    await plantMarker();

    // inner hot-updates (it self-accepts), its callback invalidates, the
    // re-walk reaches outer's live accept — but this client never received
    // outer's factory, so the coverage check turns the apply into a clean
    // full reload onto FRESH content (never a silently stale page).
    editFile('inner.js', (code) => code.replace("'inner-v1'", "'inner-v2'"));
    await expect.poll(() => page.textContent('.outer')).toBe('inner-v2');
    await expect.poll(readMarker).toBe(null);

    await waitForBuildStable();
  });

  test('invalidate bubbles to an accepting importer and hot-updates (no reload)', async () => {
    await waitForBuildStable();

    // Warm outer's factory: one edit of outer ships it (it is the
    // self-accepting boundary of its own update), giving the invalidate
    // re-walk full factory coverage from here on.
    editFile('outer.js', (code) =>
      code.replace('.textContent = inner', ".textContent = 'warmed:' + inner"),
    );
    await expect.poll(() => page.textContent('.outer')).toBe('warmed:inner-v2');
    await waitForBuildStable();

    await plantMarker();
    const runsBefore = (await readOuterRuns()).length;
    const logIndex = browserLogs.length;

    // Edit inner: inner's (pre-edit) accept callback fires with the new
    // exports and calls hot.invalidate(); the client re-walks from inner's
    // importers, reaches outer's live accept, evicts and re-runs outer —
    // all client-side, no page reload.
    editFile('inner.js', (code) => code.replace("'inner-v2'", "'inner-v3'"));
    await expect.poll(() => page.textContent('.outer')).toBe('warmed:inner-v3');

    // outer really re-ran, and nothing reloaded the page.
    expect((await readOuterRuns()).length).toBeGreaterThan(runsBefore);
    expect(await readMarker()).toBe('alive');
    // The invalidate path (not a plain dep-accept) drove the update.
    await expect
      .poll(() => browserLogs.slice(logIndex).join('\n'))
      .toMatch(/\[vite\] invalidate .*inner\.js/);

    await waitForBuildStable();
  });

  test('invalidate with no accepting importer above triggers a clean full reload', async () => {
    await waitForBuildStable();
    await plantMarker();

    // lone.js self-accepts + invalidates; its only importer (main.js) accepts
    // nothing, so the re-walk finds no boundary -> full reload, fresh content.
    editFile('lone.js', (code) => code.replace("'lone-v1'", "'lone-v2'"));
    await expect.poll(() => page.textContent('.lone')).toBe('lone-v2');
    await expect.poll(readMarker).toBe(null);

    await waitForBuildStable();
  });
});
