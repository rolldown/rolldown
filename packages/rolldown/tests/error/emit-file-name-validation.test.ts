import type { Plugin } from 'rolldown';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';

async function buildWithPlugin(plugin: Plugin) {
  try {
    const build = await rolldown({
      input: './main.js',
      cwd: import.meta.dirname,
      plugins: [plugin],
    });
    await build.generate({});
  } catch (e) {
    return e as Error;
  }
}

const INVALID_NAME_RE =
  /The "fileName" or "name" properties of emitted chunks and assets must be strings that are neither absolute nor relative paths/;

describe('emitFile rejects path-like names at emit time', () => {
  test('chunk with relative path name', async () => {
    const error = await buildWithPlugin({
      name: 'emitter',
      buildStart() {
        this.emitFile({
          type: 'chunk',
          id: './main.js',
          name: '../node_modules/some-lib/entry',
        });
      },
    });
    expect(error!.message).toMatch(INVALID_NAME_RE);
    expect(error!.message).toContain('../node_modules/some-lib/entry');
  });

  test('asset with absolute path fileName', async () => {
    const error = await buildWithPlugin({
      name: 'emitter',
      buildStart() {
        this.emitFile({
          type: 'asset',
          fileName: '/etc/owned.txt',
          source: 'x',
        });
      },
    });
    expect(error!.message).toMatch(INVALID_NAME_RE);
  });

  test('subdirectory names without a leading path fragment still work', async () => {
    const error = await buildWithPlugin({
      name: 'emitter',
      buildStart() {
        this.emitFile({
          type: 'asset',
          name: 'sub/dir/asset.txt',
          source: 'x',
        });
      },
    });
    expect(error).toBeUndefined();
  });
});
