import { describe, expect, test } from 'vitest';
import { editFile, page, plantReloadMarker, readReloadMarker, waitForBuildStable } from '~utils';

// `experimentalInlineCommonChunks` with lazy compilation. The rebuild after the lazy compile
// prints the routes' shared `shared.js` -> `leaf.js` into the route chunks, but the page gets
// lazy routes as factories from `/@vite/lazy` and never loads those chunks (see
// internal-docs/lazy-compilation/design.md). `inline-common-chunks` covers the route chunks
// that carry the inlined modules, without lazy compilation.

const sharedRuns = () =>
  page.evaluate(() => (globalThis as { __sharedRuns?: number }).__sharedRuns);

const loadedRouteChunks = () =>
  page.evaluate(() =>
    performance
      .getEntriesByType('resource')
      .map((entry) => entry.name)
      .filter((name) => /\/assets\/route-[ab]-/.test(name)),
  );

describe('inline-common-chunks-lazy', () => {
  test('lazy routes that share modules run them once, also after the rebuild', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v1');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v1');
    expect(await sharedRuns()).toBe(1);

    await waitForBuildStable();
    await page.reload({ waitUntil: 'domcontentloaded' });
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v1');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v1');
    expect(await sharedRuns()).toBe(1);
    expect(await loadedRouteChunks()).toEqual([]);
  });

  test('editing a shared module hot-updates both lazy routes', async () => {
    await plantReloadMarker();

    editFile('leaf.js', (code) => code.replace("'leaf-v1'", "'leaf-v2'"));
    await expect.poll(() => page.textContent('.a')).toBe('a:leaf-v2');
    await expect.poll(() => page.textContent('.b')).toBe('b:leaf-v2');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
