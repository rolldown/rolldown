import { describe, expect, test } from 'vitest';
import { page } from '~utils';

// Regression test for https://github.com/rolldown/rolldown/issues/9946.
//
// In full-bundle-mode (`experimental.devMode`), an imported binding used at
// MODULE INIT (top-level) inside an import cycle is emitted unbound. The served
// chunk references `B` (`export const defaults = { button: B }` in a.js) but
// never declares/imports it, so the page throws `ReferenceError: B is not
// defined` at load and `.app` never renders.
//
// `a.js` -> `b.js` -> `a.js` is the cycle; `b.js` exports `class B`, used by
// `a.js` at module init. Sibling imports used only inside function bodies bind
// correctly — only the module-init reference breaks.
describe('circular-import-binding', () => {
  test('a cyclic import used at module init is bound (#9946)', async () => {
    await expect.poll(() => page.textContent('.app')).toBe('button=B');
  });
});
