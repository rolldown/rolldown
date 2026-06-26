import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

// `lazy-init-error.js` is lazily imported and throws while initializing. On the
// first compile of the lazy chunk (the on-demand `@vite/lazy` response) the real
// module is inlined into the proxy's `lazyExports` async IIFE, so its init runs
// *synchronously* inside it:
//
//   var init_lazy_init_error_1 = createEsmInitializer(
//     "...lazy-init-error.js?rolldown-lazy=1",
//     (id) => { ...
//       const lazyExports = (async () => {
//         await (init_lazy_init_error_0(), Promise.resolve().then(() => loadExports("...lazy-init-error.js")));
//         return __rolldown_runtime__.loadExports("...lazy-init-error.js");
//       })();
//     }, 1);
//
// `init_lazy_init_error_0()` throws synchronously, so this `lazyExports` rejects
// immediately — before any consumer can attach a handler. The init error escapes
// as an unhandled promise rejection, and the consumer's `await import(...)`
// resolves as if nothing went wrong.
//
// NOTE: `test.fails` — the assertions below describe the DESIRED behavior, which
// currently fails because the bug is unfixed: the init error should surface at
// the consumer's `await import(...)` with no unhandled rejection. Once fixed, the
// body will pass; drop `.fails` to turn this into a normal regression test.
describe('lazy-init-error', () => {
  test.fails(
    'init error in a lazy module should be catchable, not unhandled',
    { retry: 0 },
    async () => {
      // The bug shows on the very first compile of the lazy chunk (the on-demand
      // `@vite/lazy` response), so navigate + click on a fresh server and do NOT
      // reload — a rebuild splits the real module into a separate chunk reached
      // via a real `await import(...)`, which is catchable.
      await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await page.click('#lazy-init-error-btn');
      await expect.poll(() => page.textContent('#lazy-init-error-status')).toBe('done');

      // Let any pending unhandled-rejection event fire before asserting.
      await page.waitForTimeout(100);

      // The consumer's try/catch should be the one that sees the init error...
      const log = (await page.textContent('#lazy-init-error-log')) ?? '';
      expect(log).toContain('caught: boom during lazy init');

      // ...and nothing should escape the lazy proxy as an unhandled rejection.
      const unhandled = (await page.textContent('#lazy-init-error-unhandled')) ?? '';
      expect(unhandled).toBe('');
    },
  );
});
