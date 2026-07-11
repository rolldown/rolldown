import { createHash } from 'node:crypto';
import { readFile, realpath } from 'node:fs/promises';
import nodePath from 'node:path';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import { createMetricsBuffer, readMetrics } from '../../svelte-transform-plugin/metrics.js';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON case as the first argument');
const {
  variant,
  componentCount,
  instrumentation,
  measureEventLoop = false,
  _corpusDirectory: corpusDirectory,
  _entryPath: entryPath,
  _selectedSourceBytes: selectedSourceBytes,
  _selectionHash: selectionHash,
} = options;
if (variant !== 'ordinary' && !/^worker-(?:[1-9]|[1-5]\d|6[0-4])$/.test(variant)) {
  throw new Error(`invalid variant: ${variant}`);
}
if (typeof instrumentation !== 'boolean' || typeof measureEventLoop !== 'boolean') {
  throw new Error('instrumentation and measureEventLoop must be booleans');
}
if (!Number.isSafeInteger(componentCount) || componentCount < 1) {
  throw new Error('componentCount must be a positive safe integer');
}
if (!Number.isSafeInteger(selectedSourceBytes) || selectedSourceBytes < 1) {
  throw new Error('_selectedSourceBytes must be a positive safe integer');
}
for (const [name, value] of [
  ['_corpusDirectory', corpusDirectory],
  ['_entryPath', entryPath],
  ['_selectionHash', selectionHash],
]) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${name} must be a non-empty string`);
  }
}

const entryDirectory = nodePath.dirname(entryPath);
const replacements = [
  [await realpath(corpusDirectory), '<svelte-corpus>'],
  [corpusDirectory, '<svelte-corpus>'],
  [await realpath(entryDirectory), '<svelte-case>'],
  [entryDirectory, '<svelte-case>'],
].sort((left, right) => right[0].length - left[0].length);
const normalize = (value) => {
  let normalized = value;
  for (const [path, replacement] of replacements) {
    normalized = normalized.replaceAll(path, replacement);
  }
  return normalized;
};
let build;
let eventLoopMonitor;

try {
  globalThis.gc?.();
  if (measureEventLoop) {
    eventLoopMonitor = monitorEventLoopDelay({ resolution: 1 });
    eventLoopMonitor.enable();
    await new Promise((resolve) => setImmediate(resolve));
  }
  const cpuStartedAt = process.cpuUsage();
  const totalStartedAt = performance.now();
  const metricsBuffer = instrumentation ? createMetricsBuffer() : undefined;
  const pluginOptions = { corpusDirectory, metricsBuffer };
  const plugin =
    variant === 'ordinary'
      ? (await import('../../svelte-transform-plugin/kernel.js')).svelteTransformPlugin(
          pluginOptions,
        )
      : (await import('../../svelte-transform-plugin/index.js')).default(pluginOptions);
  const pluginSetupFinishedAt = performance.now();

  const apiStartedAt = performance.now();
  build = await rolldown({
    cwd: corpusDirectory,
    input: entryPath,
    logLevel: 'silent',
    treeshake: false,
    external: (_source, importer) => Boolean(importer && !importer.endsWith('/entry.js')),
    plugins: [plugin],
  });
  const generateStartedAt = performance.now();
  const result = await build.generate({ format: 'esm', sourcemap: true });
  const generateFinishedAt = performance.now();
  await build.close();
  build = undefined;
  if (eventLoopMonitor) await new Promise((resolve) => setImmediate(resolve));
  const totalFinishedAt = performance.now();
  const cpu = process.cpuUsage(cpuStartedAt);

  const chunks = result.output
    .filter((output) => output.type === 'chunk')
    .sort((left, right) => left.fileName.localeCompare(right.fileName));
  const rawCodeHash = createHash('sha256');
  const codeHash = createHash('sha256');
  const rawMapHash = createHash('sha256');
  const mapHash = createHash('sha256');
  let outputCodeBytes = 0;
  let outputMapBytes = 0;
  for (const chunk of chunks) {
    const map =
      typeof chunk.map === 'string'
        ? chunk.map
        : chunk.map
          ? JSON.stringify(chunk.map)
          : await readFile(`${chunk.fileName}.map`, 'utf8').catch(() => '');
    if (!map) throw new Error(`missing generated source map for ${chunk.fileName}`);
    outputCodeBytes += Buffer.byteLength(chunk.code);
    outputMapBytes += Buffer.byteLength(map);
    for (const [hash, value] of [
      [rawCodeHash, chunk.code],
      [codeHash, normalize(chunk.code)],
      [rawMapHash, map],
      [mapHash, normalize(map)],
    ]) {
      hash.update(chunk.fileName);
      hash.update('\0');
      hash.update(value);
      hash.update('\0');
    }
  }

  console.log(
    JSON.stringify({
      variant,
      componentCount,
      instrumentation,
      selectionHash,
      expectedMatchingHandlerCalls: componentCount,
      selectedSourceBytes,
      totalElapsedMs: totalFinishedAt - totalStartedAt,
      pluginSetupElapsedMs: pluginSetupFinishedAt - totalStartedAt,
      rolldownApiElapsedMs: totalFinishedAt - apiStartedAt,
      generateElapsedMs: generateFinishedAt - generateStartedAt,
      closeElapsedMs: totalFinishedAt - generateFinishedAt,
      cpuUserMs: cpu.user / 1000,
      cpuSystemMs: cpu.system / 1000,
      finalRssBytes: process.memoryUsage.rss(),
      outputCodeBytes,
      outputMapBytes,
      outputRawCodeHash: rawCodeHash.digest('hex'),
      outputCodeHash: codeHash.digest('hex'),
      outputRawMapHash: rawMapHash.digest('hex'),
      outputMapHash: mapHash.digest('hex'),
      eventLoopDelayMs: eventLoopMonitor
        ? {
            min: eventLoopMonitor.min / 1e6,
            mean: eventLoopMonitor.mean / 1e6,
            p50: eventLoopMonitor.percentile(50) / 1e6,
            p95: eventLoopMonitor.percentile(95) / 1e6,
            p99: eventLoopMonitor.percentile(99) / 1e6,
            max: eventLoopMonitor.max / 1e6,
          }
        : undefined,
      jsMetrics: readMetrics(metricsBuffer),
    }),
  );
} finally {
  eventLoopMonitor?.disable();
  await build?.close();
}
