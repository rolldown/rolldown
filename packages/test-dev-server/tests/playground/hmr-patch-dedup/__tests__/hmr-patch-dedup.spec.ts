import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// A patch carries only the factories this client lacks or holds stale. The
// client lists the factories it holds in the patch request. The first edit of
// a chain gets the whole re-execution chain (up to the accepting boundary); a
// repeat edit of the same file gets only that file's factory, because the
// client still holds current copies of the rest.

const countFactories = (body: string) => body.match(/registerFactory\(/g)?.length ?? 0;

// Stable ids are cwd-relative paths (e.g. `playground-temp/hmr-patch-dedup/baz.js`),
// so match a factory registration by the id's file-name suffix.
const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

describe('hmr-patch-dedup', () => {
  test('first edit gets the whole chain; a repeat edit gets only the changed file', async () => {
    // Collect every HMR patch the page imports.
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

      // First edit of baz: the client holds no factories yet (the initial
      // bundle is scope-hoisted), so the patch must carry the full re-run
      // chain baz -> bar -> foo (foo is the self-accepting boundary).
      editFile('baz.js', (code) => code.replace("'baz-v1'", "'baz-v2'"));
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v2)');

      expect(patchBodies.length).toBe(1);
      const firstPatch = await patchBodies[0];
      expect(firstPatch).toMatch(factoryFor('baz.js'));
      expect(firstPatch).toMatch(factoryFor('bar.js'));
      expect(firstPatch).toMatch(factoryFor('foo.js'));
      expect(countFactories(firstPatch)).toBe(3);

      await waitForBuildStable();

      // Repeat edit of the same file: the client holds bar and foo, and they
      // are not stale, so the second patch carries baz's factory alone.
      editFile('baz.js', (code) => code.replace("'baz-v2'", "'baz-v3'"));
      await expect.poll(() => page.textContent('.chain')).toBe('bar(baz-v3)');

      expect(patchBodies.length).toBe(2);
      const secondPatch = await patchBodies[1];
      expect(secondPatch).toMatch(factoryFor('baz.js'));
      expect(secondPatch).not.toMatch(factoryFor('bar.js'));
      expect(secondPatch).not.toMatch(factoryFor('foo.js'));
      expect(countFactories(secondPatch)).toBe(1);

      await waitForBuildStable();
    } finally {
      page.off('response', onResponse);
    }
  });
});
