import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Same as `hmr-dynamic-import-self-accept`, but with `devMode.lazy: true`. `foo` compiles on
// demand behind a `?rolldown-lazy=1` proxy, which never runs in the client. Both boundary walks
// step over the proxy to `app`: the client graph links `app -> foo` directly, and the server
// walk puts `app` in the patch, so its self-accept can run it again.

describe('hmr-lazy-dynamic-import-self-accept', () => {
  test('renders the dynamically-imported value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v1');
  });

  test('editing the dynamically-imported module hot-updates via the importer self-accept', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('foo.js', (code) => code.replace("'foo-v1'", "'foo-v2'"));
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v2');

    // No full reload happened: the boundary walk crossed the lazy dynamic edge to `app`.
    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
