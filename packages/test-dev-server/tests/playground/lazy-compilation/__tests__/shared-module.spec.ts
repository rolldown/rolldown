import { describe, expect, test } from 'vitest';
import {
  editFile,
  getBuildSeq,
  page,
  serverUrl,
  waitForBuildStable,
  waitForNextBuild,
} from '~utils';

// A lazy visit starts no build, so a reload would serve the output from before the visit.
// An edit of a file of the visited route makes the server reload the page, and that page load
// builds the stale output. The new output holds the fetched proxies of the visited routes.
// The edit changes an export: an edit that changes no output, such as a comment, starts
// nothing. Edit only files that no other spec of this playground loads: every spec runs its
// own server on the same files.
async function buildAfterVisit(file: string): Promise<void> {
  const buildSeq = await getBuildSeq();
  editFile(
    file,
    (code) =>
      `${code.replace(/export const editSeq = \d+;\n$/, '')}export const editSeq = ${buildSeq};\n`,
  );
  await waitForNextBuild(buildSeq);
  await waitForBuildStable();
}

describe('lazy-shared-module', () => {
  // Regression for PR #9132: when a lazily imported module is shared by
  // several lazy entries, it lands in a common chunk where export names get
  // minified. The fetched proxy must read exports from the runtime registry
  // (`loadExports`), not from the raw chunk namespace — otherwise `sel.foo`
  // is undefined because the chunk renamed `foo` to something like `$`.
  test('preserves original export names for shared lazy module (#9132)', { retry: 0 }, async () => {
    // First load: the lazy fetches mark each proxy as fetched. No assertions
    // here — the bug only shows on an output built after the fetches.
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await page.click('#shared-module-btn');
    await expect.poll(() => page.textContent('#shared-module-status')).toBe('done');

    await buildAfterVisit('shared-module/page-a.js');

    // Second load: main.js is built around the fetched proxies — the path
    // PR #9132 fixes. Before the fix, `sel.foo` was undefined here.
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.click('#shared-module-btn');
    await expect.poll(() => page.textContent('#shared-module-status')).toBe('done');

    const log = (await page.textContent('#shared-module-log')) ?? '';
    expect(log).toContain('page-a.a = foo_value');
    expect(log).toContain('page-b.b = bar_value');
    expect(log).toContain('sel.foo = foo_value');
    expect(log).toContain('sel.bar = bar_value');
    expect(log).not.toContain('UNDEFINED');
  });

  // `selectors.js` enters the build only through lazy compilation, so the
  // dev server must add it to the file watcher when the lazy entry compiles
  // — otherwise edits to it go unnoticed. Edit the file, let the server
  // reload the page on its own, and check the new values.
  test('should watch and auto-reload a lazy-loaded module', { retry: 0 }, async () => {
    editFile('shared-module/selectors.js', (code) =>
      code
        .replace("'foo_value'", "'foo_value_updated'")
        .replace("'bar_value'", "'bar_value_updated'"),
    );

    // The previous test left the status at 'done'; a real full reload resets
    // it to the HTML default 'pending'.
    await expect
      .poll(() => page.textContent('#shared-module-status'), { timeout: 15_000 })
      .toBe('pending');

    await waitForBuildStable();

    await page.click('#shared-module-btn');
    await expect.poll(() => page.textContent('#shared-module-status')).toBe('done');

    const log = (await page.textContent('#shared-module-log')) ?? '';
    expect(log).toContain('page-a.a = foo_value_updated');
    expect(log).toContain('page-b.b = bar_value_updated');
    expect(log).toContain('sel.foo = foo_value_updated');
    expect(log).toContain('sel.bar = bar_value_updated');
    expect(log).not.toContain('UNDEFINED');
  });

  // After boot, every lazy route reaches the page as factories, and the runtime runs a module
  // once by its id. A route that was visited before the reload must not come back as built
  // code, even when the output was built after the visit: that code would run `store.js` a
  // second time and give the route its own `store`. The visited route is already in the
  // server's graph, so its second compile skips the scan and starts no build.
  test('a shared module runs once when a visited route loads after a new one', async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await page.click('#shared-module-a-btn');
    await expect.poll(() => page.textContent('#shared-module-stores')).toBe('a:1 b:- same:false');
    await buildAfterVisit('shared-module/route-a.js');

    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.click('#shared-module-b-btn');
    await expect.poll(() => page.textContent('#shared-module-stores')).toBe('a:- b:1 same:false');
    await waitForBuildStable();
    const buildSeq = await getBuildSeq();
    await page.click('#shared-module-a-btn');
    await expect.poll(() => page.textContent('#shared-module-stores')).not.toMatch(/a:-/);
    await waitForBuildStable();

    expect(await getBuildSeq()).toBe(buildSeq);
    expect(await page.textContent('#shared-module-stores')).toBe('a:1 b:1 same:true');
    expect(
      await page.evaluate(
        () => (globalThis as { __sharedModuleStoreRuns?: number }).__sharedModuleStoreRuns,
      ),
    ).toBe(1);
  });
});
