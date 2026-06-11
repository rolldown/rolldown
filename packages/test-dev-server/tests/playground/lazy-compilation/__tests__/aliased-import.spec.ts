import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// Regression for vitejs/vite#22454: with lazy compilation plus
// `viteAliasPlugin`, `import('@lazy')` got the `?rolldown-lazy=1` suffix
// twice. The doubled id broke the proxy's module bookkeeping, the real module
// never registered its exports, and `mod.foo` / `mod.bar` were undefined.
// Fixed in `lazy_compilation_plugin.rs::resolve_id` by making the suffix
// append idempotent.
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
