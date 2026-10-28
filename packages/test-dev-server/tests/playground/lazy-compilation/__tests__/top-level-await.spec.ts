import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// https://github.com/rolldown/rolldown/issues/11110
describe('lazy-compilation: top-level await', () => {
  // `retry: 0`: only the first click runs the lazy chunk. Later page loads get
  // the module from the rebuilt bundle, which does not use the factory wrapper.
  test('a lazy module with top-level await runs', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await page.click('#top-level-await-btn');
    await expect.poll(() => page.textContent('#top-level-await-status')).toBe('TLA ok');
  });

  // Pins a known gap: lazy chunks keep only the syntax of top-level await, not
  // its order. The importer reads `value` before the await sets it.
  test('an importer reads the exports before the await finishes', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await page.click('#top-level-await-tdz-btn');
    await expect
      .poll(() => page.textContent('#top-level-await-tdz-status'))
      .toBe("ReferenceError: Cannot access 'value' before initialization");
  });
});
