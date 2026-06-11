import { describe, expect, test } from 'vitest';
import { editFile, page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-shared-module', () => {
  // Regression for PR #9132: when a lazily imported module is shared by
  // several lazy entries, it lands in a common chunk where export names get
  // minified. The fetched proxy must read exports from the runtime registry
  // (`loadExports`), not from the raw chunk namespace — otherwise `sel.foo`
  // is undefined because the chunk renamed `foo` to something like `$`.
  test('preserves original export names for shared lazy module (#9132)', { retry: 0 }, async () => {
    // First load: the lazy fetches mark each proxy as fetched, and the
    // server rebuilds main.js around the fetched proxies. No assertions here
    // — the bug only shows on the rebuilt output.
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await page.click('#shared-module-btn');
    await expect.poll(() => page.textContent('#shared-module-status')).toBe('done');

    // Wait for those rebuilds to settle.
    await waitForBuildStable();

    // Second load: main.js now uses the fetched proxies — the path PR #9132
    // fixes. Before the fix, `sel.foo` was undefined here.
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
});
