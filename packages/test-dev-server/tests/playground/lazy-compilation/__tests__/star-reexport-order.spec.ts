import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// `entry-plain.js` and `entry-with-import.js` have the same two `export *` lines in the
// same order. The only difference is an unrelated `import` from b.js at the top of the
// second file. Which module owns the shared name `foo` must not depend on that line.
describe('lazy-compilation: star re-export order', () => {
  test('an unrelated import does not change which `export *` wins', async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#star-reexport-order-status')).toBe('ready');

    await page.click('#star-reexport-order-btn');
    await expect.poll(() => page.textContent('#star-reexport-order-status')).not.toBe('loading');
    expect(await page.textContent('#star-reexport-order-status')).toBe(
      'plain=from-c with-import=from-c',
    );
  });
});
