import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Port of the removed Rust fixture
// `crates/rolldown/tests/rolldown/topics/hmr/no_boundary_reload`: a single
// entry module with no acceptance — the purest no-boundary case, so any edit
// must end in a clean full reload onto fresh content. The fixture never
// executed (`expectExecuted: false`) and only asserted the server's
// full-reload decision, which now lives in the client — observable only here.
// Browser adaptation: the fixture's `console.log(1)` → `console.log(2)` edit
// becomes a DOM render of '1' → '2' so this spec can observe it.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-no-boundary-reload', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('1');
  });

  test('editing the entry module reloads onto fresh content', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('main.js', (code) => code.replace("'1'", "'2'"));
    await expect.poll(() => page.textContent('.value')).toBe('2');

    // A reload, never a silently stale page: the marker is gone AND the fresh
    // value rendered.
    await expect.poll(readMarker).toBe(null);
    await waitForBuildStable();
  });
});
