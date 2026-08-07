import { describe, expect, test } from 'vitest';
import {
  addFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  removeFile,
  waitForBuildStable,
} from '~utils';

// End-to-end check that `import.meta.glob` follows files appearing and disappearing
// (rolldown#10059). The dev engine only ever watches individual files, so this can only work if the
// native glob plugin puts the directories it walked into the watch set and then claims the owning
// module from `hotUpdate`. Under bundled dev, vite's chokidar does not drive HMR at all
// (`hmr.ts`'s `if (config.experimental.bundledDev) return`), so nothing else can cover for it.

const text = (selector: string) => page.textContent(selector);
const readRuns = () =>
  page.evaluate(() => (window as unknown as { __globRuns?: number }).__globRuns ?? -1);

describe('hmr-import-glob', () => {
  test('renders the initial glob results', async () => {
    await waitForBuildStable();
    await expect.poll(() => text('.pages')).toBe('./pages/a.js,./pages/b.js');
    await expect.poll(() => text('.titles')).toBe('a,b');
    await expect.poll(() => text('.nested')).toBe('./nested/deep/x.js');
    // `later/` does not exist yet.
    await expect.poll(() => text('.later')).toBe('');
  });

  test('a new matching file joins an eager glob', async () => {
    await waitForBuildStable();

    addFile('pages/c.js', "export const title = 'c';\n");
    await expect.poll(() => text('.pages')).toBe('./pages/a.js,./pages/b.js,./pages/c.js');
    // The glob is eager, so the new module was really imported and executed, not just listed.
    await expect.poll(() => text('.titles')).toBe('a,b,c');
    await waitForBuildStable();
  });

  test('deleting a matching file drops it from the glob', async () => {
    await waitForBuildStable();

    removeFile('pages/c.js');
    await expect.poll(() => text('.pages')).toBe('./pages/a.js,./pages/b.js');
    await expect.poll(() => text('.titles')).toBe('a,b');
    await waitForBuildStable();
  });

  test('a non-matching file in a watched directory produces no update', async () => {
    await waitForBuildStable();
    await plantReloadMarker();
    const before = await readRuns();

    // `pages/` is watched, so this event does reach the engine, but no glob matches `.txt`. Watching
    // directories must not turn every stray file into a rebuild the client can see.
    addFile('pages/notes.txt', 'not a module\n');
    await waitForBuildStable();

    expect(await readRuns()).toBe(before);
    expect(await readReloadMarker()).toBe('alive');
  });

  test('a match in a brand-new nested directory joins a `**` glob', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    // `nested/fresh/` does not exist yet, so nothing inside it can be watched. Only its own creation
    // is delivered, through the watch on `nested/`, and the round that follows re-walks the subtree
    // and picks up `y.js`, which is on disk by then.
    addFile('nested/fresh/y.js', "export const value = 'y';\n");
    await expect.poll(() => text('.nested')).toBe('./nested/deep/x.js,./nested/fresh/y.js');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('a match under a directory missing at boot joins the glob', async () => {
    await waitForBuildStable();

    // Nothing under `later/` could be watched while `later/` itself did not exist, so the plugin
    // watched the nearest existing ancestor instead. `mkdir later` is the only event available, and
    // the hook has to claim the module for it.
    addFile('later/z.js', "export const title = 'z';\n");
    await expect.poll(() => text('.later'), { timeout: 20_000 }).toBe('./later/z.js');
    await waitForBuildStable();
  });
});
