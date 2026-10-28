import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-compilation: circular named re-export', () => {
  test('initializes circular named re-exports before their importers', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#circular-named-reexport-status')).toBe('ready');

    await page.click('#circular-named-reexport-btn');
    await expect.poll(() => page.textContent('#circular-named-reexport-status')).not.toBe('loading');
    expect(await page.textContent('#circular-named-reexport-status')).toBe(
      'circ-named-ok circ-named-ok',
    );
  });
});
