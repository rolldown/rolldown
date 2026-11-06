import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { browser, editFile, page, serverUrl, waitForBuildStable } from '~utils';

// An edit adds a static import of heavy.js, which is already in the server's
// module graph but was never shipped to tab B:
//
//   ① tab A clicks -> lazy compile -> heavy.js enters the graph, its factory
//     is delivered to tab A only (lazy chunks are per-request);
//   ② tab B loads the page but never clicks -> holds no heavy.js factory;
//   ③ an edit adds `import './heavy.js'` to self-accepting hmr.js;
//   ④ the patch for tab B also carries heavy.js, so both tabs hot-update.
//
// The server-side patch shape is pinned by the
// `new_static_dep_in_unloaded_chunk` crate fixture snapshot.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = (p: typeof page) =>
  p.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = (p: typeof page) =>
  p.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

/** Collect the bodies of the HMR patches a tab fetches. */
const collectPatchBodies = (p: typeof page) => {
  const bodies: Promise<string>[] = [];
  const onResponse = (res: Response) => {
    if (/\/hmr_patch_\d+\.js(?:\?|$)/.test(res.url())) {
      bodies.push(res.text());
    }
  };
  p.on('response', onResponse);
  return { bodies, stop: () => p.off('response', onResponse) };
};

describe('hmr-new-static-dep-unloaded-chunk', () => {
  test('should render initial content without loading the lazy chunk', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('v1');
    expect(await page.textContent('.heavy')).toBe('');
  });

  test('a tab that never loaded the lazy chunk still hot-updates when an edit adds a static import of it', async () => {
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
    const patchesA = collectPatchBodies(page);
    const patchesB = collectPatchBodies(pageB);
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
      // tab B, which never received it through a lazy chunk. Tab A already
      // holds the current heavy.js, so its patch leaves it out.
      await expect.poll(() => page.textContent('.value')).toBe('v2-heavy');
      expect(await readMarker(page)).toBe('alive');
      expect(patchesA.bodies.length).toBe(1);
      expect(await patchesA.bodies[0]).not.toMatch(factoryFor('heavy.js'));

      await expect.poll(() => pageB.textContent('.value')).toBe('v2-heavy');
      expect(await readMarker(pageB)).toBe('alive');
      expect(logsB.join('\n')).not.toContain('Failed to reload');
      expect(patchesB.bodies.length).toBe(1);
      expect(await patchesB.bodies[0]).toMatch(factoryFor('heavy.js'));

      await waitForBuildStable();
    } finally {
      patchesA.stop();
      await pageB.close();
    }
  });
});
