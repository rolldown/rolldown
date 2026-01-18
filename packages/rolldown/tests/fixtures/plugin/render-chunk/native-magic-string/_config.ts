import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js'],
    experimental: {
      nativeMagicString: true,
    },
    output: {
      sourcemap: true,
    },
    plugins: [
      {
        name: 'test-render-chunk-magic-string',
        renderChunk(code, _chunk, _options, meta) {
          if (!meta?.magicString) {
            throw new Error(
              'magicString should be available when nativeMagicString is enabled',
            );
          }

          meta.magicString.replaceAll('name', 'userName');
          // Test 1: Verify that multiple accesses return the same cached instance
          const ref1 = meta.magicString;
          const ref2 = meta.magicString;
          expect(ref1 === ref2).toBe(true);

          // Test 3: Verify hasChanged() works
          expect(meta.magicString.hasChanged()).toBe(true);

          // Test 4: Verify generateMap() works with options
          const map = meta.magicString.generateMap({
            source: 'main.js',
            includeContent: true,
            hires: false,
          });
          expect(map.version).toBe(3);
          expect(map.sources).toContain('main.js');
          expect(map.sourcesContent).toBeDefined();
          expect(map.mappings).toBeDefined();
          expect(map.mappings.length).toBeGreaterThan(0);
          // Verify toString() returns valid JSON
          expect(() => JSON.parse(map.toString())).not.toThrow();

          // Return the MagicString instance directly
          return meta.magicString;
        },
      },
    ],
  },
  afterTest: function (output) {
    const code = output.output[0].code;
    const map = output.output[0].map;

    // Verify the transformations were applied
    expect(code).toContain('userName'); // 'name' was renamed to 'userName'

    // Verify sourcemap exists and has valid structure
    expect(map).toBeDefined();
    expect(map!.version).toBe(3);
    expect(map!.mappings).toBeDefined();
    expect(map!.mappings.length).toBeGreaterThan(0);

    // Verify sourcemap has sources (should reference the original file)
    expect(map!.sources).toBeDefined();
    expect(map!.sources.length).toBeGreaterThan(0);
  },
});
