import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { stripVTControlCharacters } from 'node:util';
import { rolldown, type OutputOptions, type Plugin } from 'rolldown';
import { viteReporterPlugin } from 'rolldown/experimental';
import { expect, onTestFinished, test } from 'vitest';

test.each<{
  name: string;
  sourcemap: OutputOptions['sourcemap'];
  sourcemapFileNames?: string;
  generateBundle?: Plugin['generateBundle'];
}>([
  { name: 'external map', sourcemap: true },
  { name: 'hidden map', sourcemap: 'hidden' },
  { name: 'inline map', sourcemap: 'inline' },
  {
    name: 'inline map sharing a filename with an asset',
    sourcemap: 'inline',
    sourcemapFileNames: 'metadata.json',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'metadata.json',
        source: 'x'.repeat(1000),
      });
    },
  },
  { name: 'no map', sourcemap: false },
  {
    name: 'custom map filename',
    sourcemap: true,
    sourcemapFileNames: 'maps/[name]-[hash].data',
  },
  {
    name: 'modified string map asset',
    sourcemap: true,
    sourcemapFileNames: 'maps/[name]-[hash].data',
    generateBundle(_, output) {
      const asset = Object.values(output).find((item) => item.type === 'asset')!;
      asset.source = JSON.stringify({
        ...JSON.parse(asset.source as string),
        sourcesContent: ['é'.repeat(1000)],
      });
    },
  },
  {
    name: 'modified byte map asset',
    sourcemap: 'hidden',
    generateBundle(_, output) {
      const asset = Object.values(output).find((item) => item.type === 'asset')!;
      asset.source = new TextEncoder().encode(
        JSON.stringify({
          ...JSON.parse(asset.source as string),
          sourcesContent: ['é'.repeat(1000)],
        }),
      );
    },
  },
  {
    name: 'deleted map asset',
    sourcemap: true,
    generateBundle(_, output) {
      const asset = Object.values(output).find((item) => item.type === 'asset')!;
      delete output[asset.fileName];
    },
  },
])('reports sourcemap size: $name', async ({ sourcemap, sourcemapFileNames, generateBundle }) => {
  const dir = await mkdtemp(path.join(tmpdir(), 'rolldown-vite-reporter-'));
  onTestFinished(() => rm(dir, { recursive: true, force: true }));
  const logs: string[] = [];
  const bundle = await rolldown({
    input: 'entry',
    plugins: [
      {
        name: 'test-input',
        resolveId: () => path.join(dir, 'entry.js'),
        load: () => 'console.log("hello");',
        generateBundle,
      },
      viteReporterPlugin({
        root: dir,
        isTty: false,
        isLib: false,
        assetsDir: 'assets',
        chunkLimit: 500,
        warnLargeChunks: false,
        reportCompressedSize: false,
        logInfo(message) {
          logs.push(stripVTControlCharacters(message));
        },
      }),
    ],
  });
  onTestFinished(() => bundle.close());
  const { output } = await bundle.write({ dir, sourcemap, sourcemapFileNames });
  const chunk = output.find((item) => item.type === 'chunk')!;
  const asset = output.find(
    (item) => item.type === 'asset' && item.fileName === chunk.sourcemapFileName,
  );
  expect(logs).toHaveLength(1);
  if (!sourcemap) {
    expect(logs[0]).not.toContain(' │ map:');
    return;
  }
  const source =
    sourcemap !== 'inline' && asset?.type === 'asset' ? asset.source : chunk.map!.toString();
  const size = typeof source === 'string' ? Buffer.byteLength(source) : source.byteLength;
  expect(logs[0]).toContain(` │ map: ${(Math.floor(size / 10) / 100).toFixed(2)} kB`);
});
