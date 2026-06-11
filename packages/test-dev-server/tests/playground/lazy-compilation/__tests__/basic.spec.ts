import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

describe('lazy-compilation: basic', () => {
  test('should load lazy module on demand', async () => {
    // Track JS requests to verify the lazy-loading pattern. The page is
    // navigated here (not in setup — serve.ts skips it) so the server sees a
    // cold first request, with no prior warming.
    const jsRequests: string[] = [];
    page.on('request', (req) => {
      const url = req.url();
      if (url.includes('.js')) {
        jsRequests.push(url);
      }
    });

    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });

    // 1. Verify the entry ran.
    await expect.poll(() => page.textContent('#basic-status')).toBe('main loaded');

    // 2. Trigger the lazy import and wait for it to resolve.
    await page.click('#basic-btn');
    await expect.poll(() => page.textContent('#basic-status')).toBe('lazy-loaded');

    // 3. Verify lazy compilation produced separate chunks for lazy-module
    // (proxy chunk + actual chunk with different hashes) — which eager bundling
    // would not produce.
    const lazyModuleChunks = jsRequests.filter((url) => url.includes('lazy-module'));
    expect(lazyModuleChunks.length).toBe(2);
  });
});
