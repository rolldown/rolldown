import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// A lazy request lists what the client holds, like an HMR patch request: a
// lazy response omits every module this client already ran or holds.
// Route A and route B both statically import shared.js — loaded sequentially,
// A's chunk carries shared.js's factory, B's does NOT, and B still renders
// shared values through the registry.

const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

describe('lazy-overlap-dedup', () => {
  test('the second lazy route omits the shared module the client already ran', { retry: 0 }, async () => {
    // Keyed by the lazy request's `id` param (the proxy module id).
    const lazyBodies: { id: string; body: Promise<string> }[] = [];
    const onResponse = (res: Response) => {
      const url = res.url();
      if (url.includes('/@vite/lazy?')) {
        const id = new URL(url).searchParams.get('id') ?? '';
        lazyBodies.push({ id, body: res.text() });
      }
    };
    page.on('response', onResponse);

    try {
      await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await waitForBuildStable();

      // Route A first: its chunk carries page-a AND the shared module.
      await page.click('#route-a-btn');
      await expect.poll(() => page.textContent('#route-a-content')).toBe('A:shared-value');

      // Route B second: this client already ran shared.js, so B's chunk must
      // NOT carry it again — and B still works via the registry.
      await page.click('#route-b-btn');
      await expect.poll(() => page.textContent('#route-b-content')).toBe('B:shared-value');

      expect(lazyBodies.length).toBe(2);
      const bodyA = await lazyBodies.find((e) => e.id.includes('page-a'))!.body;
      const bodyB = await lazyBodies.find((e) => e.id.includes('page-b'))!.body;

      expect(bodyA).toMatch(factoryFor('page-a.js'));
      expect(bodyA).toMatch(factoryFor('shared.js'));

      expect(bodyB).toMatch(factoryFor('page-b.js'));
      expect(bodyB).not.toMatch(factoryFor('shared.js'));
    } finally {
      page.off('response', onResponse);
    }
  });
});
