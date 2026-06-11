import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// Regression for vitejs/vite#22454. With `experimental.devMode.lazy: true` and
// `viteAliasPlugin`, `import('@lazy')` used to produce a proxy module whose id
// carried `?rolldown-lazy=1?rolldown-lazy=1` (suffix appended twice). The
// doubled key broke `delete __rolldown_runtime__.modules[$STABLE_PROXY_MODULE_ID]`
// in the proxy template (the template emits a SINGLE-suffix key), the dedup gate
// skipped the fetched-template re-execution, the real module never registered
// its named exports, and `mod.foo` / `mod.bar` came back undefined. The fix in
// `crates/rolldown_plugin_lazy_compilation/src/lazy_compilation_plugin.rs::resolve_id`
// makes the marker append idempotent.
describe('lazy-aliased-import', () => {
  test('aliased dynamic import resolves named exports (#vite-22454)', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await page.click('#aliased-import-btn');
    await expect.poll(() => page.textContent('#aliased-import-status')).toBe('done');

    const log = (await page.textContent('#aliased-import-log')) ?? '';
    expect(log).toContain('mod.foo = lazy_foo');
    expect(log).toContain('mod.bar = lazy_bar');
    expect(log).not.toContain('UNDEFINED');
  });
});
