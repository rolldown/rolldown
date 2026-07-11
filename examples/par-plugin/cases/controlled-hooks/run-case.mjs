import { createHash } from 'node:crypto';
import nodePath from 'node:path';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { setTimeout as delay } from 'node:timers/promises';
import { rolldown } from 'rolldown';
import { createMetricsBuffer, readMetrics } from '../../controlled-hooks-plugin/metrics.js';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON case as the first argument');

const {
  variant,
  hook,
  graphShape,
  moduleCount,
  workIterations,
  syncFsProbes,
  asyncDelayMs,
  resultPaddingBytes,
  instrumentation,
  measureEventLoop = false,
  _corpusDirectory: corpusDirectory,
  _entrySourceBytes: entrySourceBytes,
} = options;

if (variant !== 'ordinary' && !/^worker-(?:[1-9]|[1-5]\d|6[0-4])$/.test(variant)) {
  throw new Error(`invalid variant: ${variant}`);
}
if (hook !== 'resolveId' && hook !== 'load') throw new Error(`invalid hook: ${hook}`);
if (graphShape !== 'wide' && graphShape !== 'chain') {
  throw new Error(`invalid graph shape: ${graphShape}`);
}
for (const [name, value, minimum] of [
  ['moduleCount', moduleCount, 1],
  ['workIterations', workIterations, 0],
  ['syncFsProbes', syncFsProbes, 0],
  ['asyncDelayMs', asyncDelayMs, 0],
  ['resultPaddingBytes', resultPaddingBytes, 0],
  ['_entrySourceBytes', entrySourceBytes, 0],
]) {
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new Error(`${name} must be a safe integer >= ${minimum}`);
  }
}
if (typeof instrumentation !== 'boolean') throw new Error('instrumentation must be boolean');
if (typeof measureEventLoop !== 'boolean') throw new Error('measureEventLoop must be boolean');
if (typeof corpusDirectory !== 'string' || corpusDirectory.length === 0) {
  throw new Error('_corpusDirectory must be a non-empty string');
}

const createSupportPlugin = () => {
  if (hook === 'resolveId') {
    return {
      name: 'controlled-resolve-support-loader',
      load(id) {
        if (!id.startsWith('\0controlled-resolved:')) return null;
        const [, shape, countText, indexText, checksum] = id.split(':');
        const count = Number(countText);
        const index = Number(indexText);
        const nextImport =
          shape === 'chain' && index + 1 < count
            ? `import 'controlled-resolve:${index + 1}';\n`
            : '';
        return `${nextImport}globalThis.__controlledResolve = (globalThis.__controlledResolve || 0) + ${index} + 0x${checksum};\n`;
      },
    };
  }
  return {
    name: 'controlled-load-support-resolver',
    resolveId(specifier) {
      if (!specifier.startsWith('controlled-load:')) return null;
      return `\0${specifier}`;
    },
  };
};

let build;
let eventLoopMonitor;

try {
  process.chdir(corpusDirectory);
  globalThis.gc?.();
  if (measureEventLoop) {
    eventLoopMonitor = monitorEventLoopDelay({ resolution: 1 });
    eventLoopMonitor.enable();
    await delay(25);
    eventLoopMonitor.reset();
  }
  const cpuStartedAt = process.cpuUsage();
  const totalStartedAt = performance.now();
  const metricsBuffer = instrumentation ? createMetricsBuffer() : undefined;
  const pluginOptions = {
    metricsBuffer,
    hook,
    graphShape,
    moduleCount,
    workIterations,
    syncFsProbes,
    asyncDelayMs,
    resultPaddingBytes,
    probePath: nodePath.join(corpusDirectory, 'fs-probe.txt'),
  };
  const plugin =
    variant === 'ordinary'
      ? (await import('../../controlled-hooks-plugin/impl.js')).controlledHookPlugin(pluginOptions)
      : (await import('../../controlled-hooks-plugin/index.js')).default(pluginOptions);
  const supportPlugin = createSupportPlugin();
  const pluginSetupFinishedAt = performance.now();

  const apiStartedAt = performance.now();
  build = await rolldown({
    cwd: corpusDirectory,
    input: 'entry.js',
    logLevel: 'silent',
    treeshake: false,
    plugins: [plugin, supportPlugin],
  });
  const generateStartedAt = performance.now();
  const result = await build.generate({ format: 'esm' });
  const generateFinishedAt = performance.now();
  await build.close();
  build = undefined;
  const totalFinishedAt = performance.now();
  const cpu = process.cpuUsage(cpuStartedAt);
  if (eventLoopMonitor) {
    await delay(25);
    eventLoopMonitor.disable();
  }

  const chunks = result.output
    .filter((output) => output.type === 'chunk')
    .sort((a, b) => a.fileName.localeCompare(b.fileName));
  const rawHash = createHash('sha256');
  const normalizedHash = createHash('sha256');
  let outputBytes = 0;
  for (const chunk of chunks) {
    outputBytes += Buffer.byteLength(chunk.code);
    const normalizedCode = chunk.code
      .replaceAll(corpusDirectory, '<controlled-hooks-corpus>')
      .replaceAll(nodePath.basename(corpusDirectory), '<controlled-hooks-corpus>');
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
      hook,
      graphShape,
      moduleCount,
      workIterations,
      syncFsProbes,
      asyncDelayMs,
      resultPaddingBytes,
      instrumentation,
      measureEventLoop,
      expectedMatchingHandlerCalls: moduleCount,
      entrySourceBytes,
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
