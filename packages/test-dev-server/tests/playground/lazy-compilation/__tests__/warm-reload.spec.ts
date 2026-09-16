import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

describe('lazy-compilation: warm reload', () => {
  // After the first fetch, the rebuild gives the lazy module a chunk of its own.
  test('a reload after the first fetch loads the lazy module from the build output', async () => {
    const jsRequests: string[] = [];
    page.on('request', (req) => {
      const url = req.url();
      if (url.includes('.js')) {
        jsRequests.push(url);
      }
    });

    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
    await waitForBuildStable();
    await expect.poll(() => page.textContent('#basic-status')).toBe('main loaded');

    await page.click('#basic-btn');
    await expect.poll(() => page.textContent('#basic-status')).toBe('lazy-loaded');

    await waitForBuildStable();
    await page.reload({ waitUntil: 'domcontentloaded' });
    await expect.poll(() => page.textContent('#basic-status')).toBe('main loaded');
    jsRequests.length = 0;

    await page.click('#basic-btn');
    await expect.poll(() => page.textContent('#basic-status')).toBe('lazy-loaded');

    const lazyModuleRequests = jsRequests.filter((url) => url.includes('lazy-module'));
    expect(lazyModuleRequests).toHaveLength(1);
    expect(lazyModuleRequests[0]).not.toContain('/@vite/lazy');
  });
});
