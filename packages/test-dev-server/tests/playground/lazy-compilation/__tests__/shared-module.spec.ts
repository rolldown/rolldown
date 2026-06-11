import { describe, expect, test } from 'vitest';
import { editFile, page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-shared-module', () => {
  // Regression test for PR #9132: when a dynamically imported
  // module is shared by multiple lazy entries (so it lands in a
  // `ChunkKind::Common` chunk with minified export keys), the fetched proxy
  // must read exports from the runtime registry via `loadExports` rather than
  // returning the raw chunk namespace. Before the fix, `sel.foo` was
  // `undefined` because the namespace key was an alias like `$` instead of
  // `foo`.
  test('preserves original export names for shared lazy module (#9132)', { retry: 0 }, async () => {
    // First load: triggers initial lazy fetches through the NOT-fetched proxy
    // template. The server marks each proxy fetched and rebuilds main.js to
    // embed the *fetched* proxy chunks. We don't assert against this load — we
    // need the rebuilt output to land first, because the bug only manifests
    // through the fetched-template path.
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await page.click('#shared-module-btn');
    await expect.poll(() => page.textContent('#shared-module-status')).toBe('done');

    // Wait for the rebuild(s) triggered by mark_as_fetched to settle.
    await waitForBuildStable();

    // Second load: main.js now imports the fetched-proxy chunks for page-a,
    // page-b, and selectors. This is the path PR #9132 fixes — before the fix,
    // returning the raw chunk namespace meant `sel.foo === undefined` because
    // chunk-level export aliasing renamed `foo`/`bar` to short identifiers.
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

  // `selectors.js` is pulled into the build only via lazy compilation (the
  // initial graph contains just `main.js`; `page-a`, `page-b`, and `selectors`
  // arrive through the dynamic-import proxies). The dev coordinator must
  // register `selectors.js` with the FS watcher after `compile_lazy_entry`,
  // otherwise edits to it are invisible and no full reload is dispatched.
  // This test asserts the auto-reload path end-to-end: edit the file, let the
  // dev server reload the page on its own, and verify the new values.
  test('should watch and auto-reload a lazy-loaded module', { retry: 0 }, async () => {
    editFile('shared-module/selectors.js', (code) =>
      code
        .replace("'foo_value'", "'foo_value_updated'")
        .replace("'bar_value'", "'bar_value_updated'"),
    );

    // The previous test left `#shared-module-status` as 'done'. A successful
    // full reload resets it to the HTML default 'pending'.
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
