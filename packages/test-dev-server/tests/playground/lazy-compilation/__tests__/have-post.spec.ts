import type { Request } from 'playwright';
import { describe, expect, test } from 'vitest';
import {
  editFile,
  lazyTargetsOf,
  page,
  plantReloadMarker,
  readReloadMarker,
  serverUrl,
  waitForBuildStable,
} from '~utils';

describe('lazy-compilation: have-post', () => {
  test('a query sent in a POST body still serves a lazy load and an HMR update', async () => {
    const posts: string[] = [];
    const pullQueries: string[] = [];
    const onRequest = (req: Request) => {
      const url = new URL(req.url());
      if (req.method() === 'POST' && url.pathname.endsWith('/__rolldown_hmr_have')) {
        posts.push(req.postData() ?? '');
      } else if (url.pathname.endsWith('/__rolldown_hmr_patch.js')) {
        pullQueries.push(url.search);
      }
    };
    page.on('request', onRequest);

    try {
      await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
      await waitForBuildStable();
      // Every query is longer than 0 characters, so every request posts first.
      await page.evaluate(() => {
        (globalThis as any).__rolldown_runtime__.maxQueryLength = 0;
      });

      await page.click('#have-post-btn');
      await expect.poll(() => page.textContent('#have-post-status')).toBe('view:value-v1');
      expect(posts).toHaveLength(1);
      expect(posts[0]).toMatch(/^s=\d+&g=\d+&l=[bd][\w-]+&e=[bd][\w-]+&n=\d+$/);
      expect(await lazyTargetsOf(posts[0])).toEqual([expect.stringMatching(/have-post\/route\.js$/)]);
      expect(pullQueries).toHaveLength(1);
      expect(pullQueries[0]).toMatch(/^\?t=[^&]+&n=\d+$/);
      await waitForBuildStable();

      await plantReloadMarker();
      editFile('have-post/value.js', (code) => code.replace('value-v1', 'value-v2'));
      await expect.poll(() => page.textContent('#have-post-status')).toBe('view:value-v2');
      expect(await readReloadMarker()).toBe('alive');
      expect(posts).toHaveLength(2);
      expect(new URLSearchParams(posts[1]).get('u')).toMatch(/^[bd]/);
      expect(pullQueries).toHaveLength(2);
      expect(pullQueries[1]).toMatch(/^\?t=[^&]+&n=\d+$/);
      await waitForBuildStable();
    } finally {
      page.off('request', onRequest);
    }
  });
});
