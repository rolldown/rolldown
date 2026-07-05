import { describe, expect, test } from 'vitest';
import { page, serverUrl } from '~utils';

describe('lazy-compilation: basic', () => {
  test('should load lazy module on demand', async () => {
    // Track JS requests. Navigation happens here, not in setup (serve.ts
    // skips it), so the server sees a cold first request.
    const jsRequests: string[] = [];
    page.on('request', (req) => {
      const url = req.url();
      if (url.includes('.js')) {
        jsRequests.push(url);
      }
    });

    await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });

    // 1. The entry ran.
    await expect.poll(() => page.textContent('#basic-status')).toBe('main loaded');

    // 2. Trigger the lazy import and wait for it to resolve.
    await page.click('#basic-btn');
    await expect.poll(() => page.textContent('#basic-status')).toBe('lazy-loaded');

    // 3. The lazy module came as two separate requests (proxy chunk + real
    // chunk) — eager bundling would not do that.
    const lazyModuleChunks = jsRequests.filter((url) => url.includes('lazy-module'));
    expect(lazyModuleChunks.length).toBe(2);
  });
});
