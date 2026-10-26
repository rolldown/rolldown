// Compare two native bindings built from the PR base and head, using the same JS glue.
// node --import @oxc-node/core/register scripts/misc/fuzz-chunk-cycles.mjs BASE.node HEAD.node [seeds=100] [start=1]
// Each failure retains the seed, source files, output files, and runtime observations.
// Use --compare DIR to compare retained reports without building or executing them again.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { generateGraph } from '../../packages/rolldown/tests/src/inline-common-chunks/fuzz.ts';
import { generateDynamicGraph } from '../../packages/rolldown/tests/src/chunk-optimization/fuzz.ts';

const script = fileURLToPath(import.meta.url);
const runner = fileURLToPath(
  new URL(
    '../../packages/rolldown/tests/behaviors/inline-common-chunks/run-root.mjs',
    import.meta.url,
  ),
);
const [base, head, count = '100', start = '1'] = process.argv.slice(2);

function runNode(args, env = process.env) {
  const result = spawnSync(process.execPath, args, {
    env,
    encoding: 'utf8',
    timeout: 120_000,
    maxBuffer: 32 * 1024 * 1024,
  });
  assert.equal(result.error, undefined, String(result.error));
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return result.stdout;
}

if (base === '--worker') {
  // Set the override after the TypeScript loader loads its own native library.
  process.env.NAPI_RS_NATIVE_LIBRARY_PATH = process.env.ROLLDOWN_FUZZ_BINDING;
  const { rolldown } = await import('../../packages/rolldown/dist/index.mjs');
  delete process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
  const dir = head;
  const report = [];
  for (let seed = Number(start); seed < Number(start) + Number(count); seed++) {
    for (const kind of ['dynamic', 'general']) {
      const graph = kind === 'dynamic' ? generateDynamicGraph(seed) : generateGraph(seed);
      for (const strict of [false, true]) {
        for (const format of ['esm', 'cjs']) {
          const name = `${kind}-${seed}-${strict}-${format}`;
          const outputDir = path.join(dir, name);
          fs.mkdirSync(outputDir, { recursive: true });
          // Save the source separately so a failing graph is inspectable without the generator.
          fs.writeFileSync(
            path.join(outputDir, 'sources.json'),
            JSON.stringify(graph.modules, null, 2),
          );
          const bundle = await rolldown({
            input: graph.input,
            preserveEntrySignatures: 'strict',
            ...graph.inputOptions,
            logLevel: 'silent',
            experimental: {
              ...graph.inputOptions?.experimental,
              chunkOptimization: {
                avoidRedundantChunkLoads: true,
                mergeCommonChunks: seed % 2 === 0,
              },
              onDemandWrapping: strict && seed % 3 === 0,
            },
            plugins: [
              {
                name: 'generated-modules',
                resolveId(id) {
                  return id in graph.modules ? id : undefined;
                },
                load(id) {
                  return graph.modules[id];
                },
              },
            ],
          });
          let output;
          try {
            output = await bundle.write({
              ...graph.output,
              dir: outputDir,
              format,
              strictExecutionOrder: strict,
              entryFileNames: '[name].js',
              chunkFileNames: '[name]-[hash].js',
            });
          } finally {
            await bundle.close();
          }
          fs.writeFileSync(
            path.join(outputDir, 'package.json'),
            JSON.stringify({
              type: format === 'esm' ? 'module' : 'commonjs',
              imports: { '#ext/*': './ext-*.js' },
            }),
          );
          for (const [name, source] of Object.entries(graph.files ?? {})) {
            fs.writeFileSync(path.join(outputDir, name), source);
          }
          const chunks = output.output.filter((item) => item.type === 'chunk');
          const roots = chunks
            .filter((chunk) => chunk.isEntry)
            .map((chunk) => chunk.fileName)
            .sort();
          assert.ok(roots.length > 1);
          const scenarios = [...roots.map((root) => [root]), roots, [...roots].reverse()];
          const observations = scenarios.map((roots) =>
            JSON.parse(runNode([runner, ...roots.map((root) => path.join(outputDir, root))])),
          );
          const hashes = chunks
            .map((chunk) => [chunk.fileName, createHash('sha256').update(chunk.code).digest('hex')])
            .sort(([a], [b]) => a.localeCompare(b));
          const item = { name, hashes, scenarios, observations };
          fs.writeFileSync(
            path.join(outputDir, 'observations.json'),
            JSON.stringify(item, null, 2),
          );
          report.push(item);
        }
      }
    }
    if ((seed - Number(start) + 1) % 10 === 0) process.stderr.write(`Completed seed ${seed}\n`);
  }
  fs.writeFileSync(path.join(dir, 'report.json'), JSON.stringify(report));
} else {
  let dir;
  if (base === '--compare') {
    assert.ok(head, 'Provide the directory with retained reports.');
    dir = path.resolve(head);
  } else {
    assert.ok(base && head, 'Provide the base and head native binding paths.');
    assert.ok(Number.isSafeInteger(Number(count)) && Number(count) > 0, 'Invalid seed count');
    assert.ok(Number.isSafeInteger(Number(start)) && Number(start) > 0, 'Invalid starting seed');
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-chunk-cycles-'));
    console.log(`Artifacts: ${dir}`);
    // Separate processes ensure each build loads exactly one revision of the native library.
    for (const [name, binding] of [
      ['base', base],
      ['head', head],
    ]) {
      const outputDir = path.join(dir, name);
      fs.mkdirSync(outputDir);
      const result = spawnSync(
        process.execPath,
        [...process.execArgv, script, '--worker', outputDir, count, start],
        {
          env: { ...process.env, ROLLDOWN_FUZZ_BINDING: path.resolve(binding) },
          stdio: 'inherit',
        },
      );
      assert.equal(result.status, 0, `${name} worker failed; artifacts: ${dir}`);
    }
  }
  const read = (name) => JSON.parse(fs.readFileSync(path.join(dir, name, 'report.json'), 'utf8'));
  const before = read('base');
  const after = read('head');
  assert.equal(after.length, before.length);
  let runs = 0;
  let successfulRuns = 0;
  const codeDifferences = [];
  for (let i = 0; i < before.length; i++) {
    const { hashes: beforeHashes, ...beforeRuntime } = before[i];
    const { hashes: afterHashes, ...afterRuntime } = after[i];
    assert.deepEqual(afterRuntime, beforeRuntime, `${before[i].name}; artifacts: ${dir}`);
    // Report code differences for inspection. An identical revision can assign different
    // internal export aliases, so byte equality alone cannot decide runtime equivalence.
    if (JSON.stringify(beforeHashes) !== JSON.stringify(afterHashes)) {
      codeDifferences.push(before[i].name);
    }
    runs += after[i].scenarios.length;
    successfulRuns += after[i].observations.filter((result) => result.error === null).length;
  }
  console.log(
    JSON.stringify(
      {
        buildsPerRevision: after.length,
        runtimeComparisons: runs,
        successfulRuns,
        matchingErrorRuns: runs - successfulRuns,
        identicalOutput: codeDifferences.length === 0,
        codeDifferences,
        artifacts: dir,
      },
      null,
      2,
    ),
  );
}
