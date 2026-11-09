import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { browser, editFile, page, serverUrl, waitForBuildStable } from '~utils';

// A patch can carry a module the tab never ran. Here tab B receives route.js
// in the patch for an edit of shared.js (route.js imports shared.js), but
// never receives dep.js. The later lazy chunk for route.js must still carry
// dep.js: the server walks through route.js (shipped, but never run) instead
// of stopping there.
//
//   setup ──import()──▶ holder ──static──▶ shared
//     └────import()──▶ route ──static──▶ shared
//                           └──static──▶ dep
//
// Tab B runs holder.js so that it applies the patch: the client imports a
// patch only when the tab ran one of the changed modules.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = (p: typeof page) =>
  p.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = (p: typeof page) =>
  p.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

const factoryFor = (file: string) =>
  new RegExp(`registerFactory\\("[^"]*walk-through-shipped/${file}"`);

describe('lazy-compilation: walk-through-shipped', () => {
  test('a lazy chunk carries the deps of a module that a patch shipped but never ran', async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();

    const pageB = await browser.newPage();
    const patchBodiesB: Promise<string>[] = [];
    const routeChunkBodiesB: Promise<string>[] = [];
    pageB.on('response', (res: Response) => {
      const url = decodeURIComponent(res.url());
      if (/\/hmr_patch_\d+\.js(?:\?|$)/.test(url)) {
        patchBodiesB.push(res.text());
      } else if (url.includes('/@vite/lazy?') && url.includes('walk-through-shipped/route.js')) {
        routeChunkBodiesB.push(res.text());
      }
    });
    const pageErrorsB: string[] = [];
    pageB.on('pageerror', (err) => pageErrorsB.push(err.message));
    try {
      // Tab B runs holder.js and shared.js, but not route.js.
      await pageB.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await pageB.click('#walk-through-shipped-holder-btn');
      await expect.poll(() => pageB.textContent('#walk-through-shipped-holder')).toBe('shared-v1');
      await waitForBuildStable();

      // Tab A opens the route: route.js enters the server's module graph.
      await page.click('#walk-through-shipped-route-btn');
      await expect
        .poll(() => page.textContent('#walk-through-shipped-route'))
        .toBe('shared-v1+dep');
      await waitForBuildStable();
      await plantMarker(pageB);

      // The edit hot-updates holder.js in tab B. The patch also carries
      // route.js (an importer of shared.js), which tab B has never run.
      editFile('walk-through-shipped/shared.js', (code) =>
        code.replace("'shared-v1'", "'shared-v2'"),
      );
      await expect.poll(() => pageB.textContent('#walk-through-shipped-holder')).toBe('shared-v2');
      expect(patchBodiesB.length).toBe(1);
      const patchB = await patchBodiesB[0];
      expect(patchB).toMatch(factoryFor('route.js'));
      expect(patchB).not.toMatch(factoryFor('dep.js'));
      await waitForBuildStable();

      // Tab B opens the route: route.js runs from the patch's factory, and
      // dep.js must come with the lazy chunk. Tab B holds the current
      // route.js and shared.js from the patch, so the walk goes through them
      // without carrying them.
      await pageB.click('#walk-through-shipped-route-btn');
      await expect
        .poll(() => pageB.textContent('#walk-through-shipped-route'))
        .toBe('shared-v2+dep');
      const routeChunksB = await Promise.all(routeChunkBodiesB);
      expect(routeChunksB.some((body) => factoryFor('dep.js').test(body))).toBe(true);
      expect(routeChunksB.some((body) => factoryFor('route.js').test(body))).toBe(false);
      expect(routeChunksB.some((body) => factoryFor('shared.js').test(body))).toBe(false);
      expect(await readMarker(pageB)).toBe('alive');
      expect(pageErrorsB).toEqual([]);
    } finally {
      await pageB.close();
    }
  });
});
