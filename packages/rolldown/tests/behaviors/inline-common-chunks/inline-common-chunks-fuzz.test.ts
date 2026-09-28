import { describe, expect, test } from 'vitest';
import { generateGraph } from '../../src/inline-common-chunks/fuzz';
import {
  assertRegistrationOrder,
  buildBoth,
  rootFiles,
  runRoots,
  type RootResult,
} from '../../src/inline-common-chunks/harness';

// Generated module graphs and option sets, built with `experimentalInlineCommonChunks` off and
// on. Every entry runs as the root of a fresh Node process, alone and after each other entry, and
// both builds must log, export and fail the same way. `INLINE_COMMON_CHUNKS_FUZZ_SEEDS=<n>` runs more seeds;
// a failing seed is reproducible from its number alone.
const seedCount = Number(process.env.INLINE_COMMON_CHUNKS_FUZZ_SEEDS ?? 12);
const seeds = Array.from({ length: seedCount }, (_, i) => i + 1);

/**
 * Deconflicted identifiers (`C$2`) differ between the builds, and an error message can name one
 * (`C$2 is not a constructor`); nothing else may differ, so only messages are normalized. A
 * minified build renames every local, so there every identifier in a message is masked.
 */
function comparable(result: RootResult, minified: boolean): unknown {
  const normalize = (error: unknown): unknown => {
    if (error === null || typeof error !== 'object' || !('message' in error)) return error;
    let message = String(error.message).replace(/\$\d+/g, '');
    if (minified) message = message.replace(/[A-Za-z_$][\w$]*/g, '_');
    return { ...error, message };
  };
  return {
    ...result,
    error: normalize(result.error),
    roots: result.roots.map((root) => ({ ...root, error: normalize(root.error) })),
  };
}

describe('experimentalInlineCommonChunks on generated graphs', () => {
  test.each(seeds)(
    'seed %i',
    async (seed) => {
      const graph = generateGraph(seed);
      const pair = await buildBoth(`fuzz-${seed}`, {
        input: graph.input,
        modules: graph.modules,
        files: graph.files,
        inputOptions: { logLevel: 'silent', ...graph.inputOptions },
        output: graph.output,
      });
      if (!graph.minified) assertRegistrationOrder(pair.on);
      const roots = rootFiles(pair.off);
      const scenarios: string[][] = roots.map((root) => [root]);
      for (const first of roots) {
        for (const second of roots) {
          if (first !== second) scenarios.push([first, second]);
        }
      }
      for (const scenario of scenarios) {
        expect(
          comparable(runRoots(pair.on, scenario), graph.minified),
          `seed ${seed}, roots ${scenario.join(' then ')}`,
        ).toEqual(comparable(runRoots(pair.off, scenario), graph.minified));
      }
    },
    60_000,
  );
});
