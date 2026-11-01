import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { rolldown, type OutputBundle, type OutputChunk, type Plugin } from 'rolldown';
import { expect, test } from 'vitest';

const virtualEntry: Plugin = {
  name: 'virtual-entry',
  resolveId: (id) => id,
  load: () => 'console.log("entry");',
  buildStart() {
    this.emitFile({
      type: 'asset',
      fileName: 'bytes.bin',
      source: new Uint8Array([1, 2, 3]),
    });
  },
};

function chunkOf(bundle: OutputBundle): OutputChunk {
  return Object.values(bundle).find((item): item is OutputChunk => item.type === 'chunk')!;
}

test('nested output edits reach later output hooks, written files, and the returned bundle', async () => {
  const seen: string[] = [];
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-output-mutations-'));
  const build = await rolldown({
    input: 'entry',
    plugins: [
      virtualEntry,
      {
        name: 'mutate-in-generate',
        generateBundle(_options, bundle) {
          const asset = bundle['bytes.bin'];
          if (asset.type !== 'asset') throw new Error('Expected asset');
          const source = asset.source as Uint8Array;
          expect(source).toBe(asset.source);
          expect(source).toBeInstanceOf(Uint8Array);
          expect(ArrayBuffer.isView(source)).toBe(true);
          source[0] = 9;
          source.subarray(1, 2).fill(8);
          new DataView(source.buffer, source.byteOffset, source.byteLength).setUint8(2, 7);
          const chunk = chunkOf(bundle);
          const map = chunk.map!;
          expect(map).toBe(chunk.map);
          expect(map.sources).toBe(map.sources);
          map.sources[0] = 'changed.js';
          map.sourcesContent[0] = 'console.log("changed");';
          map.names.push('changed');
          map.file = 'changed.js';
          map.x_google_ignoreList = [0];
          expect(JSON.parse(map.toString()).sources).toEqual(['changed.js']);
        },
      },
      {
        name: 'read-in-generate',
        generateBundle(_options, bundle) {
          const asset = bundle['bytes.bin'];
          if (asset.type !== 'asset') throw new Error('Expected asset');
          expect(Array.from(asset.source as Uint8Array)).toEqual([9, 8, 7]);
          expect(chunkOf(bundle).map).toMatchObject({
            file: 'changed.js',
            sources: ['changed.js'],
            sourcesContent: ['console.log("changed");'],
            names: ['changed'],
            ignoreList: [0],
          });
          seen.push('generate');
        },
        writeBundle(_options, bundle) {
          const asset = bundle['bytes.bin'];
          if (asset.type !== 'asset') throw new Error('Expected asset');
          (asset.source as Uint8Array).set([6], 0);
          chunkOf(bundle).map!.sources[0] = 'written.js';
        },
      },
      {
        name: 'read-in-write',
        writeBundle(_options, bundle) {
          const asset = bundle['bytes.bin'];
          if (asset.type !== 'asset') throw new Error('Expected asset');
          expect(Array.from(asset.source as Uint8Array)).toEqual([6, 8, 7]);
          expect(chunkOf(bundle).map!.sources).toEqual(['written.js']);
          seen.push('write');
        },
      },
    ],
  });
  try {
    const { output } = await build.write({ dir, sourcemap: true });
    expect(seen).toEqual(['generate', 'write']);
    expect(Array.from(fs.readFileSync(path.join(dir, 'bytes.bin')))).toEqual([9, 8, 7]);
    const asset = output.find((item) => item.type === 'asset' && item.fileName === 'bytes.bin')!;
    expect(asset.type === 'asset' && Array.from(asset.source as Uint8Array)).toEqual([6, 8, 7]);
    expect(output[0].map!.sources).toEqual(['written.js']);
  } finally {
    await build.close();
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('in-place output metadata edits reach later plugins', async () => {
  const build = await rolldown({
    input: 'entry',
    plugins: [
      virtualEntry,
      {
        name: 'mutate-metadata',
        generateBundle(_options, bundle) {
          const chunk = chunkOf(bundle);
          chunk.imports.push('added.js');
          chunk.dynamicImports.push('dynamic.js');
          const asset = bundle['bytes.bin'];
          if (asset.type !== 'asset') throw new Error('Expected asset');
          asset.names.push('bytes');
          asset.originalFileNames.push('original.bin');
        },
      },
      {
        name: 'read-metadata',
        generateBundle(_options, bundle) {
          expect(chunkOf(bundle).imports).toEqual(['added.js']);
          expect(chunkOf(bundle).dynamicImports).toEqual(['dynamic.js']);
          expect(bundle['bytes.bin']).toMatchObject({
            names: ['bytes'],
            originalFileNames: ['original.bin'],
          });
        },
      },
    ],
  });
  try {
    const { output } = await build.generate({});
    expect(output[0].imports).toEqual(['added.js']);
    expect(output[0].dynamicImports).toEqual(['dynamic.js']);
  } finally {
    await build.close();
  }
});
