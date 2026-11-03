import { setTimeout } from 'node:timers/promises';
import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// A page reload mints a fresh clientId, so the server's per-client delivery
// ship map resets: the next edit ships the FULL chain of factories again (the
// new session holds none), and still applies as a hot update.

const countFactories = (body: string) => body.match(/registerFactory\(/g)?.length ?? 0;
const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

const plantMarker = () => page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-reconnect-ship-map', () => {
  test('a reload resets the ship map: the next edit re-ships the whole chain and still hot-updates', async () => {
    const patchBodies: Promise<string>[] = [];
    const onResponse = (res: Response) => {
      if (/\/hmr_patch_\d+\.js(?:\?|$)/.test(res.url())) {
        patchBodies.push(res.text());
      }
    };
    page.on('response', onResponse);

    try {
      await waitForBuildStable();
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v1)');

      // First edit: hot update; the fresh session receives the whole chain.
      editFile('baz.js', (code) => code.replace("'baz-v1'", "'baz-v2'"));
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');
      expect(patchBodies.length).toBe(1);
      expect(countFactories(await patchBodies[0])).toBe(3);
      await waitForBuildStable();

      // Reload: fresh clientId -> the server-side ship map for this tab resets.
      await page.reload();
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');
      await waitForBuildStable();
      // Let the debounced post-regeneration reload (if any) land before we
      // start measuring hot-vs-reload with the marker.
      await setTimeout(600);
      await plantMarker();

      // Second edit, post-reload: still a HOT update (marker survives), and
      // the patch carries the full chain again — bar's and foo's factories
      // are re-shipped because this session never received them.
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
