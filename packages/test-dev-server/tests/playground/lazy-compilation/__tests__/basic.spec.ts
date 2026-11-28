import { describe, expect, test } from 'vitest';
import { isPullUrl, lazyTargetsOf, page, serverUrl, waitForBuildStable } from '~utils';

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
    await waitForBuildStable();

    // 1. The entry ran.
    await expect.poll(() => page.textContent('#basic-status')).toBe('main loaded');

    // 2. Trigger the lazy import and wait for it to resolve.
    await page.click('#basic-btn');
    await expect.poll(() => page.textContent('#basic-status')).toBe('lazy-loaded');

    // 3. The lazy module was fetched on demand, in exactly one request — the
    // pull whose `l` names it, which compiles it. Eager bundling would have
    // shipped it inside the entry and made no request at all; a stub chunk
    // standing in for it would have made two. The request names the module by
    // its number, never by its path.
    const pulls = jsRequests.filter(isPullUrl);
    const lazyTargets = await Promise.all(pulls.map(lazyTargetsOf));
    expect(lazyTargets).toEqual([[expect.stringMatching(/lazy-module\.js$/)]]);
    expect(jsRequests.filter((url) => url.includes('lazy-module'))).toEqual([]);
  });
});
