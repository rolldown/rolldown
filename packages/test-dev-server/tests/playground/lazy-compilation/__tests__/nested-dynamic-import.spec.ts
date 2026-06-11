import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// Regression for `hmr_ast_finalizer.rs::try_rewrite_dynamic_import` (the
// `?rolldown-lazy=1` branch). `outer.js` is itself a lazy chunk and contains
// `await import('./inner.js')`, which is also lazy. Before the fix, that
// inner import returned a namespace without the `'rolldown:exports'` key, so
// `inner.foo` came back undefined. The fix loads the proxy module's
// registered exports instead.
describe('lazy-nested-dynamic-import', () => {
  // `retry: 0` matters: the bug only shows on the first click of a fresh
  // page. That click makes the server rebuild main.js to use the fetched
  // proxy, which bypasses the buggy code path — so a retry would always pass
  // and hide the regression.
  test('lazy chunk can dynamically import another lazy chunk', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await page.click('#nested-dynamic-import-btn');
    await expect.poll(() => page.textContent('#nested-dynamic-import-status')).toBe('done');

    const log = (await page.textContent('#nested-dynamic-import-log')) ?? '';
    expect(log).toContain('outer.outerName = outer');
    expect(log).toContain('inner.foo = inner_foo');
    expect(log).toContain('inner.bar = inner_bar');
    expect(log).not.toContain('UNDEFINED');
  });
});
