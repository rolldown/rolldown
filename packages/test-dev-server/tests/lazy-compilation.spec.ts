import { describe, expect, test } from 'vitest';
import { getLazyPage } from './test-utils';

describe('lazy-compilation', () => {
  test.sequential('should load lazy module on demand', async () => {
    const page = getLazyPage();

    // Track JS requests to verify lazy loading pattern
    const jsRequests: string[] = [];
    page.on('request', (req: { url: () => string }) => {
      const url = req.url();
      if (url.endsWith('.js')) {
        jsRequests.push(url);
      }
    });

    // Reload the page to capture requests from the beginning
    await page.reload({ waitUntil: 'domcontentloaded' });

    // 1. Verify main module loaded
    await expect.poll(() => page.textContent('.status')).toBe('main loaded');

    // 2. Wait for lazy module to load (triggered by setTimeout in main.js)
    await expect.poll(() => page.textContent('.lazy-result')).toBe('lazy-loaded');

    // 3. Verify lazy compilation produced separate chunks:
    // - One for main.js
    // - Two for lazy-module (proxy chunk + actual chunk with different hashes)
    // This would NOT happen with eager bundling where everything is in one bundle.
    const lazyModuleChunks = jsRequests.filter((url) => url.includes('lazy-module'));
    expect(lazyModuleChunks.length).toBe(2);
  });
});
