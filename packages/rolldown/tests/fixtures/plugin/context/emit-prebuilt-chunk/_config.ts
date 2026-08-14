import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-emit-prebuilt-chunk',
        buildStart() {
          // Prebuilt chunk without custom options (defaults)
          this.emitFile({
            type: 'prebuilt-chunk',
            fileName: 'prebuilt.js',
            code: 'console.log("prebuilt chunk");',
            exports: ['default'],
          });
          // Prebuilt chunk with options
          this.emitFile({
            type: 'prebuilt-chunk',
            fileName: 'vendor-abc123.js',
            name: 'vendor',
            code: 'console.log("vendor chunk");',
            exports: ['foo'],
            isEntry: true,
            isDynamicEntry: true,
            facadeModuleId: '/path/to/entry.js',
          });
        },
      },
    ],
  },
  afterTest: (output) => {
    const chunks = getOutputChunk(output);

    // Test prebuilt chunk without custom options (defaults)
    const prebuiltChunk = chunks.find((c) => c.fileName === 'prebuilt.js');
    expect(prebuiltChunk).toBeDefined();
    expect(prebuiltChunk!.code).toBe('console.log("prebuilt chunk");');
    expect(prebuiltChunk!.exports).toStrictEqual(['default']);
    expect(prebuiltChunk!.name).toBe('prebuilt.js'); // name defaults to fileName
    expect(prebuiltChunk!.isEntry).toBe(false);
    expect(prebuiltChunk!.isDynamicEntry).toBe(false);
    expect(prebuiltChunk!.facadeModuleId).toBeNull();

    // Test prebuilt chunk with options
    const vendorChunk = chunks.find((c) => c.fileName === 'vendor-abc123.js');
    expect(vendorChunk).toBeDefined();
    expect(vendorChunk!.code).toBe('console.log("vendor chunk");');
    expect(vendorChunk!.exports).toStrictEqual(['foo']);
    expect(vendorChunk!.name).toBe('vendor');
    expect(vendorChunk!.isEntry).toBe(true);
    expect(vendorChunk!.isDynamicEntry).toBe(true);
    expect(vendorChunk!.facadeModuleId).toBe('/path/to/entry.js');
  },
});
