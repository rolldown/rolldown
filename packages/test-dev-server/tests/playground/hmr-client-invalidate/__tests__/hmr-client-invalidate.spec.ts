import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { browserLogs, editFile, page, waitForBuildStable } from '~utils';

// `import.meta.hot.invalidate()` is handled by the client: it walks again
// from the invalidator's importers and applies the update locally. The server
// never hears about the invalidate; the client only requests the factories it
// lacks, the same way it does for a pushed update.
//
// An edit of `inner` brings only inner's factory (inner accepts itself), so
// the first invalidate requests outer's factory. The client keeps it, so a
// later invalidate needs no request for outer.

const plantMarker = () => page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);
const readOuterRuns = () =>
  page.evaluate(() => (window as unknown as { __outerRuns?: string[] }).__outerRuns ?? []);

const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

/** Collect the bodies of the HMR patches the page fetches. */
const collectPatchBodies = () => {
  const bodies: Promise<string>[] = [];
  const onResponse = (res: Response) => {
    if (/\/__rolldown_hmr_patch\.js\?/.test(res.url())) {
      bodies.push(res.text());
    }
  };
  page.on('response', onResponse);
  return { bodies, stop: () => page.off('response', onResponse) };
};

describe('hmr-client-invalidate', () => {
  test('should render initial content', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.outer')).toBe('inner-v1');
    await expect.poll(() => page.textContent('.lone')).toBe('lone-v1');
  });

  test('invalidate requests the factory of the accepting importer and hot-updates', async () => {
    await waitForBuildStable();
    await plantMarker();
    const patches = collectPatchBodies();
    const runsBefore = (await readOuterRuns()).length;
    const logIndex = browserLogs.length;

    try {
      // inner hot-updates (it self-accepts), its callback invalidates, and the
      // walk reaches outer's accept. The client never received outer's
      // factory, so it requests it, then runs outer again in place.
      editFile('inner.js', (code) => code.replace("'inner-v1'", "'inner-v2'"));
      await expect.poll(() => page.textContent('.outer')).toBe('inner-v2');

      expect((await readOuterRuns()).length).toBeGreaterThan(runsBefore);
      expect(await readMarker()).toBe('alive');
      await expect
        .poll(() => browserLogs.slice(logIndex).join('\n'))
        .toMatch(/\[vite\] invalidate .*inner\.js/);
      expect((await Promise.all(patches.bodies)).join('\n')).toMatch(factoryFor('outer.js'));
    } finally {
      patches.stop();
    }

    await waitForBuildStable();
  });

  test('a later invalidate reuses the factory the client holds', async () => {
    await waitForBuildStable();
    await plantMarker();
    const patches = collectPatchBodies();
    const runsBefore = (await readOuterRuns()).length;

    try {
      editFile('inner.js', (code) => code.replace("'inner-v2'", "'inner-v3'"));
      await expect.poll(() => page.textContent('.outer')).toBe('inner-v3');

      expect((await readOuterRuns()).length).toBeGreaterThan(runsBefore);
      expect(await readMarker()).toBe('alive');
      // outer did not change, so its factory is still current
      expect((await Promise.all(patches.bodies)).join('\n')).not.toMatch(factoryFor('outer.js'));
    } finally {
      patches.stop();
    }

    await waitForBuildStable();
  });

  test('invalidate with no accepting importer above triggers a clean full reload', async () => {
    await waitForBuildStable();
    await plantMarker();

    // lone.js self-accepts + invalidates; its only importer (main.js) accepts
    // nothing, so the re-walk finds no boundary -> full reload, fresh content.
    editFile('lone.js', (code) => code.replace("'lone-v1'", "'lone-v2'"));
    await expect.poll(() => page.textContent('.lone')).toBe('lone-v2');
    await expect.poll(readMarker).toBe(null);

    await waitForBuildStable();
  });
});
