import { createHash } from 'node:crypto';
import nodePath from 'node:path';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import { createMetricsBuffer, readMetrics } from '../../parallel-vue-plugin/metrics.js';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON case as the first argument');

const {
  variant,
  instrumentation,
  measureEventLoop = false,
  _corpusDirectory: corpusDirectory,
  _entryPaths: entryPaths,
  _sfcCount: sfcCount,
  _totalSfcBytes: totalSfcBytes,
  _manifestHash: manifestHash,
} = options;
if (
  variant !== 'full-ordinary' &&
  variant !== 'ordinary' &&
  !/^worker-(?:[1-9]|[1-5]\d|6[0-4])$/.test(variant)
) {
  throw new Error(`invalid variant: ${variant}`);
}
if (typeof instrumentation !== 'boolean') throw new Error('instrumentation must be boolean');
if (typeof corpusDirectory !== 'string' || !Array.isArray(entryPaths)) {
  throw new Error('corpus metadata is missing');
}

let build;
let eventLoopMonitor;
try {
  process.chdir(corpusDirectory);
  globalThis.gc?.();
  if (measureEventLoop) {
    eventLoopMonitor = monitorEventLoopDelay({ resolution: 1 });
    eventLoopMonitor.enable();
    await new Promise((resolve) => setImmediate(resolve));
  }
  const cpuStartedAt = process.cpuUsage();
  const totalStartedAt = performance.now();
  const metricsBuffer = instrumentation ? createMetricsBuffer() : undefined;
  const pluginOptions = { root: corpusDirectory, metricsBuffer };
  let plugin;
  if (variant === 'full-ordinary') {
    const Vue = (await import('unplugin-vue/rolldown')).default;
    plugin = Vue({
      root: corpusDirectory,
      isProduction: true,
      sourceMap: false,
      inlineTemplate: true,
    });
  } else if (variant === 'ordinary') {
    plugin = (await import('../../parallel-vue-plugin/impl.js')).vueTransformPlugin(pluginOptions);
  } else {
    plugin = (await import('../../parallel-vue-plugin/index.js')).default(pluginOptions);
  }
  const pluginSetupFinishedAt = performance.now();

  const input = Object.fromEntries(
    entryPaths.map((path) => {
      const basename = nodePath.basename(path, nodePath.extname(path));
      const name =
        basename === 'index' && path !== 'src/index.ts'
          ? nodePath.basename(nodePath.dirname(path))
          : basename;
      return [name, path];
    }),
  );
  const apiStartedAt = performance.now();
  build = await rolldown({
    cwd: corpusDirectory,
    input,
    external: ['vue'],
    logLevel: 'silent',
    moduleTypes: { vue: 'js' },
    treeshake: false,
    plugins: [plugin],
  });
  const generateStartedAt = performance.now();
  const result = await build.generate({
    format: 'esm',
    entryFileNames: '[name].js',
    chunkFileNames: 'chunks/[name]-[hash].js',
  });
  const generateFinishedAt = performance.now();
  await build.close();
  build = undefined;
  if (eventLoopMonitor) await new Promise((resolve) => setImmediate(resolve));
  const totalFinishedAt = performance.now();
  const cpu = process.cpuUsage(cpuStartedAt);

  const rawHash = createHash('sha256');
  const normalizedHash = createHash('sha256');
  let outputBytes = 0;
  let outputChunks = 0;
  let outputAssets = 0;
  let totalExports = 0;
  for (const output of [...result.output].sort((left, right) =>
    left.fileName.localeCompare(right.fileName),
  )) {
    const source = output.type === 'chunk' ? output.code : String(output.source);
    const normalizedSource = source
      .replaceAll(corpusDirectory, '<vue-icon-corpus>')
      .replaceAll(nodePath.dirname(corpusDirectory), '<vue-icon-upstream>');
    outputBytes += Buffer.byteLength(source);
    if (output.type === 'chunk') {
      outputChunks++;
      totalExports += output.exports.length;
    } else {
      outputAssets++;
    }
    for (const [hash, content] of [
      [rawHash, source],
      [normalizedHash, normalizedSource],
    ]) {
      hash.update(output.type);
      hash.update('\0');
      hash.update(output.fileName);
      hash.update('\0');
      hash.update(content);
      hash.update('\0');
    }
  }

  console.log(
    JSON.stringify({
      variant,
      instrumentation,
      expectedMatchingHandlerCalls: sfcCount,
      totalSourceBytes: totalSfcBytes,
      manifestHash,
      totalElapsedMs: totalFinishedAt - totalStartedAt,
      pluginSetupElapsedMs: pluginSetupFinishedAt - totalStartedAt,
      rolldownApiElapsedMs: totalFinishedAt - apiStartedAt,
      generateElapsedMs: generateFinishedAt - generateStartedAt,
      closeElapsedMs: totalFinishedAt - generateFinishedAt,
      cpuUserMs: cpu.user / 1000,
      cpuSystemMs: cpu.system / 1000,
      finalRssBytes: process.memoryUsage.rss(),
      outputBytes,
      outputChunks,
      outputAssets,
      totalExports,
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
