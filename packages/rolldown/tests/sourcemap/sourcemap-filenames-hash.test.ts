import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

async function generateMapName(
  sourcemapPathTransform?: (source: string, sourcemapPath: string) => string,
): Promise<string> {
  const bundle = await rolldown({ input: './main.js', cwd: import.meta.dirname });
  const { output } = await bundle.generate({
    sourcemap: true,
    sourcemapFileNames: '[name]-[hash].js.map',
    sourcemapPathTransform,
  });
  await bundle.close();
  const map = output.find((o) => o.type === 'asset' && o.fileName.endsWith('.map'));
  expect(map).toBeDefined();
  return map!.fileName;
}

test('sourcemap-filenames [hash] derives from the emitted map contents', async () => {
  // `sourcemapPathTransform` changes the emitted map's `sources`, so it must change `[hash]` —
  // the hash is a cache key for the bytes actually written to disk.
  const plain = await generateMapName();
  const transformedA = await generateMapName((source) => `transformed-a/${source}`);
  const transformedB = await generateMapName((source) => `transformed-b/${source}`);
  expect(transformedA).not.toBe(plain);
  expect(transformedA).not.toBe(transformedB);
  // Identical configuration must stay deterministic.
  expect(await generateMapName((source) => `transformed-a/${source}`)).toBe(transformedA);
});
