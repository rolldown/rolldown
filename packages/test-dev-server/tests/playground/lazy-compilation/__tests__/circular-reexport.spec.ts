import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-compilation: circular re-export', () => {
  test('initializes circular star re-exports before their importers', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#circular-reexport-status')).toBe('ready');

    await page.click('#circular-reexport-btn');
    await expect.poll(() => page.textContent('#circular-reexport-status')).not.toBe('loading');
    expect(await page.textContent('#circular-reexport-status')).toBe(
      'circ-dep-init-a circ-dep-init-b',
    );
  });
});
