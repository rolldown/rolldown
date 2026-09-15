import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// https://github.com/rolldown/rolldown/issues/9946
describe('lazy-compilation: circular import binding', () => {
  test('a cyclic import used at module init is bound', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#circular-import-binding-status')).toBe('ready');

    await page.click('#circular-import-binding-btn');
    await expect.poll(() => page.textContent('#circular-import-binding-status')).not.toBe('loading');
    expect(await page.textContent('#circular-import-binding-status')).toBe('button=B');
  });
});
