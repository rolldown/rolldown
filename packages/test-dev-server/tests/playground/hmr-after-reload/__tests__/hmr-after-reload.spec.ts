import { setTimeout } from 'node:timers/promises';
import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// A reloaded page holds no factories, so the patch for the next edit carries
// the FULL chain of factories again, and still applies as a hot update.

const countFactories = (body: string) => body.match(/registerFactory\(/g)?.length ?? 0;
const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

const plantMarker = () => page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-after-reload', () => {
  test('after a reload, the next edit gets the whole chain again and still hot-updates', async () => {
    const patchBodies: Promise<string>[] = [];
    const onResponse = (res: Response) => {
      if (/\/__rolldown_hmr_patch\.js\?/.test(res.url())) {
        patchBodies.push(res.text());
      }
    };
    page.on('response', onResponse);

    try {
      await waitForBuildStable();
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v1)');

      // First edit: hot update; the fresh page gets the whole chain.
      editFile('baz.js', (code) => code.replace("'baz-v1'", "'baz-v2'"));
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');
      expect(patchBodies.length).toBe(1);
      expect(countFactories(await patchBodies[0])).toBe(3);
      await waitForBuildStable();

      // Reload: the page loses the factories it held.
      await page.reload();
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');
      await waitForBuildStable();
      // Let the debounced post-regeneration reload (if any) land before we
      // start measuring hot-vs-reload with the marker.
      await setTimeout(600);
      await plantMarker();

      // Second edit, post-reload: still a HOT update (marker survives), and
      // the patch carries the full chain again — bar's and foo's factories
      // come again because the reloaded page does not hold them.
      editFile('baz.js', (code) => code.replace("'baz-v2'", "'baz-v3'"));
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v3)');
      expect(await readMarker()).toBe('alive');

      expect(patchBodies.length).toBe(2);
      const postReloadPatch = await patchBodies[1];
      expect(postReloadPatch).toMatch(factoryFor('baz.js'));
      expect(postReloadPatch).toMatch(factoryFor('bar.js'));
      expect(postReloadPatch).toMatch(factoryFor('foo.js'));
      expect(countFactories(postReloadPatch)).toBe(3);

      await waitForBuildStable();
    } finally {
      page.off('response', onResponse);
    }
  });
});
