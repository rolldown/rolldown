import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-compilation: circular namespace re-export', () => {
  test('initializes circular namespace re-exports before their importers', { retry: 0 }, async () => {
    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#circular-namespace-reexport-status')).toBe('ready');

    await page.click('#circular-namespace-reexport-btn');
    await expect.poll(() => page.textContent('#circular-namespace-reexport-status')).not.toBe(
      'loading',
    );
    expect(await page.textContent('#circular-namespace-reexport-status')).toBe(
      'circ-ns-ok circ-ns-ok',
    );
  });
});
