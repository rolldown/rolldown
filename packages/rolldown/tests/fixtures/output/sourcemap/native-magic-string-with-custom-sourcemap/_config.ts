import MagicString from 'magic-string';
import { defineTest } from 'rolldown-tests';
import { getLocation, getOutputAsset, getOutputChunk } from 'rolldown-tests/utils';
import { SourceMapConsumer } from 'source-map';
import { expect } from 'vitest';

// Simulates the barrier scenario: two native-MagicString plugins with a
// non-MagicString plugin in between that returns a custom sourcemap.
// The chain must flush at the barrier and produce correct mappings.
export default defineTest({
  sequential: true,
  config: {
    input: ['main.js'],
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      // Plugin 1: uses native MagicString to rename 'foo' → 'bar'
      {
        name: 'plugin-native-1',
        transform(code, id, meta) {
          if (id.startsWith('\0')) return null;
          if (!meta?.magicString) return null;
          meta.magicString.replace('foo', 'bar');
          return { code: meta.magicString };
        },
      },
      // Plugin 1b: second consecutive native MagicString — renames 'aaa' → 'bbb'.
      // This tests that the chain composes multiple native MS steps before flushing.
      {
        name: 'plugin-native-1b',
        transform(code, id, meta) {
          if (id.startsWith('\0')) return null;
          if (!meta?.magicString) return null;
          meta.magicString.replace('aaa', 'bbb');
          return { code: meta.magicString };
        },
      },
      // Plugin 2: returns a custom sourcemap (the barrier).
      // Uses JS MagicString to produce a precise per-token sourcemap.
      {
        name: 'plugin-custom-sourcemap',
        transform(code, id) {
          if (id.startsWith('\0')) return null;
          const idx = code.indexOf('hello');
          if (idx === -1) return null;
          const s = new MagicString(code);
          s.overwrite(idx, idx + 5, 'world');
          return {
            code: s.toString(),
            map: s.generateMap({ source: id, includeContent: true, hires: true }),
          };
        },
      },
      // Plugin 3: uses native MagicString to rename 'bar' → 'baz'
      {
        name: 'plugin-native-2',
        transform(code, id, meta) {
          if (id.startsWith('\0')) return null;
          if (!meta?.magicString) return null;
          meta.magicString.replace('bar', 'baz');
          return { code: meta.magicString };
        },
      },
    ],
  },
  afterTest: async function (output) {
    const chunk = getOutputChunk(output)[0];
    // foo→bar→baz, hello→world, aaa→bbb
    expect(chunk.code).toContain('baz');
    expect(chunk.code).toContain('world');
    expect(chunk.code).toContain('bbb');
    expect(chunk.map).toBeDefined();

    const map = getOutputAsset(output)[0].source as string;
    const smc = await new SourceMapConsumer(JSON.parse(map));

    // 'baz' was originally 'foo' on line 1.
    // Traces through: chain2 (baz→bar) → barrier (identity) → chain1 (bar→foo).
    const bazLoc = getLocation(chunk.code, chunk.code.indexOf('baz'));
    const bazOriginal = smc.originalPositionFor(bazLoc);
    expect(bazOriginal.source).toContain('main.js');
    expect(bazOriginal.line).toBe(1);

    // 'world' was originally 'hello' on line 1.
    // Traces through: chain2 (identity) → barrier (world→hello) → chain1.
    const worldLoc = getLocation(chunk.code, chunk.code.indexOf('world'));
    const worldOriginal = smc.originalPositionFor(worldLoc);
    expect(worldOriginal.source).toContain('main.js');
    expect(worldOriginal.line).toBe(1);

    // 'bbb' was originally 'aaa' on line 2.
    // Verifies the chain composed two native MS steps (foo→bar, aaa→bbb)
    // before flushing at the barrier.
    const bbbLoc = getLocation(chunk.code, chunk.code.indexOf('bbb'));
    const bbbOriginal = smc.originalPositionFor(bbbLoc);
    expect(bbbOriginal.source).toContain('main.js');
    expect(bbbOriginal.line).toBe(2);
  },
});
