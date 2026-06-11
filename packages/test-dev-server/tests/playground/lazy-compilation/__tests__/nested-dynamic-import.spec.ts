import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// Regression test for the fix in
// `crates/rolldown/src/hmr/hmr_ast_finalizer.rs::try_rewrite_dynamic_import`
// (the `?rolldown-lazy=1` branch). `outer.js` is itself loaded as a lazy chunk,
// and its body contains `await import('./inner.js')`, which also resolves to a
// lazy proxy. Before the fix the HMR AST finalizer rewrote that inner dynamic
// import to a plain `import('/@vite/lazy?id=...')`, returning the partial
// bundle's raw namespace — which has no `'rolldown:exports'` key, so
// `__unwrap_lazy_compilation_entry` fell through and `inner.foo` came back
// `undefined`. The fix chains `.then(() => loadExports("<stable_proxy_id>"))`
// so the namespace read by the unwrap helper is the proxy module's registered
// exports (which carry the `'rolldown:exports'` getter).
describe('lazy-nested-dynamic-import', () => {
  // `retry: 0` is critical: the bug only manifests on the first click of a fresh
  // page. The first click triggers `mark_as_fetched`, which rebuilds main.js so
  // it references the *fetched* proxy chunk — and the fetched proxy chunk imports
  // the full-build outer.js (compiled by scope_finalizer), bypassing the buggy
  // HMR-finalizer rewrite entirely. With retries enabled, the reload between
  // attempts would always land on the post-rebuild fetched-proxy path and mask
  // the regression.
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
