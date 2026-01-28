import { defineTest } from 'rolldown-tests';
import { getOutputAsset } from 'rolldown-tests/utils';
import { expect } from 'vitest';

// Test that [name] in assetFileNames includes directory part
// https://github.com/rolldown/rolldown/issues/7315
export default defineTest({
  config: {
    output: {
      assetFileNames: '[name].[ext]',
    },
    plugins: [
      {
        name: 'test-plugin',
        buildStart() {
          // Emit asset with directory in name (valid - not a path fragment)
          this.emitFile({
            type: 'asset',
            name: 'foo/bar.txt',
            source: 'hello world',
          });
          // Emit asset with nested directory in name (valid)
          this.emitFile({
            type: 'asset',
            name: 'a/b/c.txt',
            source: 'nested',
          });
          // Path traversal in middle is allowed (will be normalized)
          // "foo/../bar/baz.txt" -> "bar/baz.txt" after normalize
          this.emitFile({
            type: 'asset',
            name: 'foo/../bar/baz.txt',
            source: 'path traversal in middle',
          });
        },
      },
    ],
  },
  afterTest: (output) => {
    const assets = getOutputAsset(output);
    expect(assets.length).toBe(3);

    const fooBarAsset = assets.find((a) => a.name === 'foo/bar.txt');
    expect(fooBarAsset).toBeDefined();
    // The [name] placeholder should include the directory part
    expect(fooBarAsset!.fileName).toBe('foo/bar.txt');

    const nestedAsset = assets.find((a) => a.name === 'a/b/c.txt');
    expect(nestedAsset).toBeDefined();
    expect(nestedAsset!.fileName).toBe('a/b/c.txt');

    // Path traversal in middle should be resolved by normalize()
    const bazAsset = assets.find((a) => a.name === 'foo/../bar/baz.txt');
    expect(bazAsset).toBeDefined();
    // "foo/.." resolves to "", leaving "bar/baz.txt"
    expect(bazAsset!.fileName).toBe('bar/baz.txt');
  },
});
