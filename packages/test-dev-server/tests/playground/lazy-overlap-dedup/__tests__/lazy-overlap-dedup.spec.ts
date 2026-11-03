import { setTimeout } from 'node:timers/promises';
import type { Response } from 'playwright';
import { describe, expect, test } from 'vitest';
import { page, serverUrl, waitForBuildStable } from '~utils';

// Lazy chunks are sized by the same per-client ship map as HMR
// patches: a lazy response omits every factory this client already received.
// Route A and route B both statically import shared.js — loaded sequentially,
// A's chunk carries shared.js's factory, B's does NOT, and B still renders
// shared values through the registry.

const factoryFor = (file: string) => new RegExp(`registerFactory\\("[^"]*/${file}"`);

describe('lazy-overlap-dedup', () => {
  test('the second lazy route omits the already-delivered shared factory', { retry: 0 }, async () => {
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

      // Route A first: its chunk delivers page-a AND the shared module.
      await page.click('#route-a-btn');
      await expect.poll(() => page.textContent('#route-a-content')).toBe('A:shared-value');

      // The ship map records a delivery when the response completes; give the
      // (async, in-process) ship-map write a beat before the next compile reads it.
      await setTimeout(300);

      // Route B second: shared.js was already delivered to this client, so
      // B's chunk must NOT re-ship it — and B still works via the registry.
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
