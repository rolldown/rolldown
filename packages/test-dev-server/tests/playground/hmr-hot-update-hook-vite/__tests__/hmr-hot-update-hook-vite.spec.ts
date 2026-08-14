import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// End-to-end check of VITE's `hotUpdate` / `handleHotUpdate` plugin contracts
// running on rolldown's `hotUpdate` hook through the bundled-dev adapter (see
// dev.config.mjs). A full reload wipes `window.__marker`, so a surviving
// marker proves the update was applied (or skipped) in place. The plugins
// assert the contract shapes themselves and throw on mismatch, which fails
// the specs below through missed accept counts.

const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);
const readAcceptCount = () =>
  page.evaluate(() => (window as unknown as { __acceptCount?: number }).__acceptCount ?? -1);
const readCustomPayload = () =>
  page.evaluate(
    () =>
      (window as unknown as { __customPayload?: { content?: string } })
        .__customPayload?.content ?? null,
  );

describe('hmr-hot-update-hook-vite', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('dep-v1');
    await expect.poll(() => page.textContent('.part')).toBe('part:part-v1');
    expect(await readAcceptCount()).toBe(0);
  });

  test('replace: editing config.txt hot-updates dep.js via the vite hotUpdate hook', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('config.txt', (code) => code.replace('config-v1', 'config-v2'));
    // The vite hook returned [dep node], so the patch re-runs dep.js and
    // main's accept callback for './dep.js' fires exactly once.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('invalidate buffering: editing invalidate.txt ships the invalidated module', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('invalidate.txt', (code) =>
      code.replace('invalidate-v1', 'invalidate-v2'),
    );
    // The vite hook buffered dep.js via `moduleGraph.invalidateModule` and
    // returned [] — the adapter's finalize hook must merge the buffer, so the
    // patch ships dep.js and the accept callback fires.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('custom protocol: editing custom.txt sends read() content over hot.send and suppresses', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('custom.txt', (code) => code.replace('custom-v1', 'custom-v2'));
    // The hook read the edited content via ctx.read(), pushed it through a
    // custom hot.send event (the client listener stores it), and returned []
    // to suppress the default update. The facade-read asserts (by-file
    // lookup, url/file/info, importedModules incl. the dynamic edge) run in
    // the same branch — a throw there means no payload ever arrives.
    await expect.poll(readCustomPayload).toContain('custom-v2');

    expect(await readAcceptCount()).toBe(before); // suppressed
    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('legacy shared ctx: plugin A reassigns read, plugin B observes it', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('chain.txt', (code) => code.replace('chain-v1', 'chain-v2'));
    // Plugin B only returns [dep] after asserting it received plugin A's
    // reassigned read on the shared HmrContext — the accept bump is the
    // proof the legacy shared-context protocol held.
    await expect.poll(readAcceptCount).toBe(before + 1);

    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  test('suppress: editing suppress.txt produces no update via legacy handleHotUpdate', async () => {
    await waitForBuildStable();
    await plantMarker();
    const before = await readAcceptCount();

    editFile('suppress.txt', (code) =>
      code.replace('suppress-v1', 'suppress-v2'),
    );
    // The legacy hook returned [], so this build round must end in a Noop:
    // same accept count, no reload. waitForBuildStable synchronizes on the
    // server's build state instead of sleeping.
    await waitForBuildStable();

    expect(await readAcceptCount()).toBe(before);
    expect(await readMarker()).toBe('alive');
  });

  test('by-file expansion: editing widget.js lets the hook ship only its query-variant module', async () => {
    await waitForBuildStable();
    await plantMarker();
    const runsBefore = await page.evaluate(
      () => (window as unknown as { __widgetRuns?: number }).__widgetRuns ?? 0,
    );

    editFile('widget.js', (code) => code.replace('part-v1', 'part-v2'));
    // The hook asserted ctx.modules contains BOTH widget.js and
    // widget.js?part=extra (upstream getModulesByFile parity) and returned
    // only the sub-module — its load() re-reads the base file, so the part
    // text updates.
    await expect.poll(() => page.textContent('.part')).toBe('part:part-v2');

    // the base module must NOT have re-run (only the sub-module shipped)
    const runsAfter = await page.evaluate(
      () => (window as unknown as { __widgetRuns?: number }).__widgetRuns ?? 0,
    );
    expect(runsAfter).toBe(runsBefore);
    expect(await readMarker()).toBe('alive'); // no full reload
    await waitForBuildStable();
  });

  // Keep this test LAST — the reload resets all window state.
  test('invalidateAll: editing reload.txt full-reloads the page', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('reload.txt', (code) => code.replace('reload-v1', 'reload-v2'));
    // The hook called moduleGraph.invalidateAll(), which maps to a full
    // rebuild + page reload under bundled dev — the reload wipes the marker.
    await expect.poll(readMarker).toBe(null);
    await expect.poll(() => page.textContent('.value')).toBe('dep-v1');
    await waitForBuildStable();
  });
});
