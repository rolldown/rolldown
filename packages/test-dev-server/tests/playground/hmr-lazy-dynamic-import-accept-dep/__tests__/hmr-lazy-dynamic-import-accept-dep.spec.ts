import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Same as `hmr-dynamic-import-accept-dep`, but with `devMode.lazy: true`. `foo` compiles on
// demand behind a `?rolldown-lazy=1` proxy, which never runs in the client. Both boundary walks
// step over the proxy to `app`, whose accept-dep boundary takes the update.

describe('hmr-lazy-dynamic-import-accept-dep', () => {
  test('renders the dynamically-imported value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v1');
  });

  test('editing the dynamically-imported dep hot-updates via the importer accept-dep', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('foo.js', (code) => code.replace("'foo-v1'", "'foo-v2'"));
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v2');

    // No full reload happened: the boundary walk crossed the lazy dynamic edge to `app`.
    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
