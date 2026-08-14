import { describe, expect, test } from 'vitest';
import { browser, editFile, page, serverUrl, waitForBuildStable } from '~utils';

// KNOWN HOLE — rolldown#10164 second-round review, focus item 1.
//
// The patch superset (upward importer walk ∪ graph-new modules) never walks
// DOWN from a changed module, so a NEWLY-ADDED static dep ships only if it
// happens to be graph-new. With lazy compilation the single-tab case is
// covered by exactly that accident: the lazy module enters the graph at the
// same rebuild that adds the edge. The MULTI-TAB variant is broken:
//
//   ① tab A clicks -> lazy compile -> heavy.js enters the graph, its factory
//     is delivered to tab A only (lazy chunks are per-request);
//   ② tab B loads the page but never clicks -> holds no heavy.js factory;
//   ③ an edit adds `import './heavy.js'` to self-accepting hmr.js -> heavy.js
//     is in the graph and NOT new -> the patch ships only hmr.js to BOTH tabs;
//   ④ tab A re-runs hmr.js fine (heavy.js resident); tab B's re-run hits
//     `initModule('heavy.js')` with no factory -> MissingFactoryError
//     mid-apply, reported via `warnFailedUpdate` -> stale tab, no reload.
//
// The skipped test asserts the DESIRED behavior: tab B hot-updates like tab A.
// Un-skip it when the patch also ships newly-added static deps that no payload
// has carried to the client yet. Until then the server-side patch shape is
// pinned by the `new_static_dep_in_unloaded_chunk` crate fixture snapshot.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = (p: typeof page) =>
  p.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = (p: typeof page) =>
  p.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-new-static-dep-unloaded-chunk', () => {
  test('should render initial content without loading the lazy chunk', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('v1');
    expect(await page.textContent('.heavy')).toBe('');
  });

  test.skip('a tab that never loaded the lazy chunk still hot-updates when an edit adds a static import of it', async () => {
    await waitForBuildStable();

    // ① Tab A triggers the lazy compile: heavy.js enters the server's graph
    // and its factory is delivered to tab A only.
    await page.click('.load-heavy');
    await expect.poll(() => page.textContent('.heavy')).toBe('heavy');
    await waitForBuildStable();

    // ② Tab B: same app, never clicks — no heavy.js factory in this tab.
    const pageB = await browser.newPage();
    const logsB: string[] = [];
    pageB.on('console', (msg) => logsB.push(msg.text()));
    try {
      await pageB.goto(serverUrl);
      await expect.poll(() => pageB.textContent('.value')).toBe('v1');
      await waitForBuildStable();
      await plantMarker(page);
      await plantMarker(pageB);

      // ③ The edit adds a NEW static edge to the already-in-graph heavy.js.
      editFile('hmr.js', (code) =>
        code.replace(
          "export const value = 'v1';",
          "import { heavy } from './heavy.js';\nexport const value = 'v2-' + heavy;",
        ));

      // ④ Both tabs hot-update: the patch must carry heavy.js's factory to
      // tab B, which never received it through a lazy chunk.
      await expect.poll(() => page.textContent('.value')).toBe('v2-heavy');
      expect(await readMarker(page)).toBe('alive');

      await expect.poll(() => pageB.textContent('.value')).toBe('v2-heavy');
      expect(await readMarker(pageB)).toBe('alive');
      expect(logsB.join('\n')).not.toContain('Failed to reload');

      await waitForBuildStable();
    } finally {
      await pageB.close();
    }
  });
});
