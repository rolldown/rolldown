import { describe, expect, test } from 'vitest';
import {
  addFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  removeFile,
  waitForBuildStable,
} from '~utils';

// Under bundled dev, vite's chokidar does not drive HMR (`hmr.ts` returns early on
// `experimental.bundledDev`). A new file reaches a glob only because the native plugin watches the
// directory it walked and claims the file from `hotUpdate`.

const text = (selector: string) => page.textContent(selector);
const readRuns = () =>
  page.evaluate(() => (window as unknown as { __globRuns?: number }).__globRuns ?? -1);

describe('hmr-import-glob', () => {
  test('renders the initial glob results', async () => {
    await waitForBuildStable();
    await expect.poll(() => text('.pages')).toBe('./pages/a.js,./pages/b.js');
    await expect.poll(() => text('.titles')).toBe('a,b');
    await expect.poll(() => text('.nested')).toBe('./nested/deep/x.js');
    // `later/` and `missing/` do not exist yet.
    await expect.poll(() => text('.later')).toBe('');
    await expect.poll(() => text('.deep')).toBe('');
  });

  test('a new matching file joins an eager glob', async () => {
    await waitForBuildStable();

    addFile('pages/c.js', "export const title = 'c';\n");
    await expect.poll(() => text('.pages')).toBe('./pages/a.js,./pages/b.js,./pages/c.js');
    // The glob is eager, so the new module was executed, not only listed.
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

    // `pages/` is watched, so the event reaches the engine, but no glob matches `.txt`.
    addFile('pages/notes.txt', 'not a module\n');
    await waitForBuildStable();

    expect(await readRuns()).toBe(before);
    expect(await readReloadMarker()).toBe('alive');
  });

  test('a match in a brand-new nested directory joins a `**` glob', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    addFile('nested/fresh/y.js', "export const value = 'y';\n");
    await expect.poll(() => text('.nested')).toBe('./nested/deep/x.js,./nested/fresh/y.js');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('a match under a directory missing at boot joins the glob', async () => {
    await waitForBuildStable();

    addFile('later/z.js', "export const title = 'z';\n");
    await expect.poll(() => text('.later'), { timeout: 20_000 }).toBe('./later/z.js');
    await waitForBuildStable();
  });

  test('a match under a directory whose parent is missing at boot joins the glob', async () => {
    await waitForBuildStable();

    addFile('missing/deep/w.js', "export const title = 'w';\n");
    await expect.poll(() => text('.deep'), { timeout: 20_000 }).toBe('./missing/deep/w.js');
    await waitForBuildStable();
  });
});
