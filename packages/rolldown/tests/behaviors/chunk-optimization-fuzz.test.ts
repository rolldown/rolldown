import fs from 'node:fs';
import path from 'node:path';
import { afterAll, expect, test } from 'vitest';
import { generateDynamicGraph } from '../src/chunk-optimization/fuzz';
import { buildCase, rootFiles, runRoots } from '../src/inline-common-chunks/harness';

// Increase CHUNK_CYCLE_FUZZ_SEEDS for local stress runs. Every seed runs against native source
// modules as well as output with the reduction disabled, so two broken bundles cannot agree.
const seeds = Array.from(
  { length: Number(process.env.CHUNK_CYCLE_FUZZ_SEEDS ?? 8) },
  (_, index) => index + 1,
);
let changedLayouts = 0;
afterAll(() => {
  expect(changedLayouts, 'the generated corpus must exercise chunk reduction').toBeGreaterThan(0);
});

test.each(seeds)(
  'dynamic chunk cycles preserve runtime behavior, seed %i',
  async (seed) => {
    const graph = generateDynamicGraph(seed);
    const build = (enabled: boolean) =>
      buildCase(`chunk-cycles-${seed}-${enabled}`, 'off', {
        ...graph,
        inputOptions: {
          logLevel: 'silent',
          preserveEntrySignatures: 'strict',
          experimental: {
            chunkOptimization: { mergeCommonChunks: false, avoidRedundantChunkLoads: enabled },
          },
        },
        output: { strictExecutionOrder: true, keepNames: true },
      });
    const off = await build(false);
    const on = await build(true);
    const placement = (built: typeof on) =>
      JSON.stringify(
        built.chunks.map((chunk) => JSON.stringify([...chunk.moduleIds].sort())).sort(),
      );
    if (placement(off) !== placement(on)) changedLayouts++;
    const source = { dir: path.join(on.dir, 'source') };
    fs.mkdirSync(source.dir, { recursive: true });
    fs.writeFileSync(path.join(source.dir, 'package.json'), '{"type":"module"}');
    for (const [name, code] of Object.entries(graph.modules)) {
      fs.writeFileSync(path.join(source.dir, name), code);
    }
    const roots = rootFiles(off);
    const scenarios = [...roots.map((root) => [root]), roots, [...roots].reverse()];
    for (const scenario of scenarios) {
      const expected = runRoots(source, scenario);
      expect(expected.error, `source failed: seed ${seed}, ${scenario}`).toBeNull();
      expect(runRoots(off, scenario), `disabled: seed ${seed}, ${scenario}`).toEqual(expected);
      expect(runRoots(on, scenario), `enabled: seed ${seed}, ${scenario}`).toEqual(expected);
    }
  },
  60_000,
);
