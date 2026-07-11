import { createHash } from 'node:crypto';
import nodePath from 'node:path';
import { performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import { createMetricsBuffer, readMetrics } from '../../controlled-transform-plugin/metrics.js';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON case as the first argument');

const {
  variant,
  graphShape,
  moduleCount,
  minimumSourceBytes,
  workIterations,
  resultPaddingBytes,
  instrumentation,
  _corpusDirectory: corpusDirectory,
  _totalSourceBytes: totalSourceBytes,
} = options;
if (variant !== 'ordinary' && !/^worker-(?:[1-9]|[1-5]\d|6[0-4])$/.test(variant)) {
  throw new Error(`invalid variant: ${variant}`);
}
if (graphShape !== 'wide' && graphShape !== 'chain') {
  throw new Error(`invalid graph shape: ${graphShape}`);
}
for (const [name, value, minimum] of [
  ['moduleCount', moduleCount, 1],
  ['minimumSourceBytes', minimumSourceBytes, 0],
  ['workIterations', workIterations, 0],
  ['resultPaddingBytes', resultPaddingBytes, 0],
  ['_totalSourceBytes', totalSourceBytes, 0],
]) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${name} must be a safe integer >= ${minimum}`);
  }
}
if (typeof corpusDirectory !== 'string' || corpusDirectory.length === 0) {
  throw new Error('_corpusDirectory must be a non-empty string');
}

let build;

try {
  process.chdir(corpusDirectory);
  globalThis.gc?.();
  const cpuStartedAt = process.cpuUsage();
  const totalStartedAt = performance.now();
  const metricsBuffer = instrumentation ? createMetricsBuffer() : undefined;
  const pluginOptions = { metricsBuffer, workIterations, resultPaddingBytes };
  const plugin =
    variant === 'ordinary'
      ? (await import('../../controlled-transform-plugin/impl.js')).controlledTransformPlugin(
          pluginOptions,
        )
      : (await import('../../controlled-transform-plugin/index.js')).default(pluginOptions);
  const pluginSetupFinishedAt = performance.now();

  const apiStartedAt = performance.now();
  build = await rolldown({
    cwd: corpusDirectory,
    input: 'entry.controlled.js',
    logLevel: 'silent',
    treeshake: false,
    plugins: [plugin],
  });
  const generateStartedAt = performance.now();
  const result = await build.generate({ format: 'esm' });
  const generateFinishedAt = performance.now();
  await build.close();
  build = undefined;
  const totalFinishedAt = performance.now();
  const cpu = process.cpuUsage(cpuStartedAt);

  const chunks = result.output
    .filter((output) => output.type === 'chunk')
    .sort((a, b) => a.fileName.localeCompare(b.fileName));
  const rawHash = createHash('sha256');
  const normalizedHash = createHash('sha256');
  let outputBytes = 0;
  for (const chunk of chunks) {
    outputBytes += Buffer.byteLength(chunk.code);
    const normalizedCode = chunk.code
      .replaceAll(corpusDirectory, '<controlled-corpus>')
      .replaceAll(nodePath.basename(corpusDirectory), '<controlled-corpus>');
    for (const [hash, code] of [
      [rawHash, chunk.code],
      [normalizedHash, normalizedCode],
    ]) {
      hash.update(chunk.fileName);
      hash.update('\0');
      hash.update(code);
      hash.update('\0');
    }
  }

  console.log(
    JSON.stringify({
      variant,
      graphShape,
      moduleCount,
      minimumSourceBytes,
      workIterations,
      resultPaddingBytes,
      instrumentation,
      expectedMatchingHandlerCalls: moduleCount + 1,
      totalSourceBytes,
      totalElapsedMs: totalFinishedAt - totalStartedAt,
      pluginSetupElapsedMs: pluginSetupFinishedAt - totalStartedAt,
      rolldownApiElapsedMs: totalFinishedAt - apiStartedAt,
      generateElapsedMs: generateFinishedAt - generateStartedAt,
      closeElapsedMs: totalFinishedAt - generateFinishedAt,
      cpuUserMs: cpu.user / 1000,
      cpuSystemMs: cpu.system / 1000,
      finalRssBytes: process.memoryUsage.rss(),
      outputBytes,
      outputRawHash: rawHash.digest('hex'),
      outputHash: normalizedHash.digest('hex'),
      jsMetrics: readMetrics(metricsBuffer),
    }),
  );
} finally {
  await build?.close();
}
