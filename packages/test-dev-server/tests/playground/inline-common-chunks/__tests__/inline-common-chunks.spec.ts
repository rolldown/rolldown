import { describe, expect, test } from 'vitest';
import { editFile, page, plantReloadMarker, readReloadMarker, waitForBuildStable } from '~utils';

// Lazy compilation compiles `route-a` and `route-b` on first load. The rebuild that follows
// makes their shared `shared.js` -> `leaf.js` a small common chunk, which
// `experimentalInlineCommonChunks` prints into both route chunks. An edit to `leaf.js` then has
// to reach the routes' self-accept boundaries through the inlined modules.

describe('inline-common-chunks', () => {
  test('both route chunks carry the shared modules after the lazy rebuild', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v1');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v1');

    // Load the output of the rebuild around the compiled routes.
    await waitForBuildStable();
    await page.reload({ waitUntil: 'domcontentloaded' });
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v1');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v1');

    const carriers = await page.evaluate(async () => {
      const urls = performance
        .getEntriesByType('resource')
        .map((entry) => entry.name)
        .filter((name) => /\/assets\/route-[ab]-/.test(name));
      const sources = await Promise.all(urls.map((url) => fetch(url).then((res) => res.text())));
      return sources.filter((source) => source.includes('__share(')).length;
    });
    expect(carriers).toBe(2);
  });

  test('editing a module inside the inlined chunk hot-updates both routes', async () => {
    await plantReloadMarker();

    editFile('leaf.js', (code) => code.replace("'leaf-v1'", "'leaf-v2'"));
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v2');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v2');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
