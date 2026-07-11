import { createHash } from 'node:crypto';
import { readFile, realpath } from 'node:fs/promises';
import nodePath from 'node:path';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import { createMetricsBuffer, readMetrics } from '../../svelte-registry-graph-plugin/metrics.js';
import { createRegistryGraphResolver, isBareDependency } from './resolver.mjs';

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON graph case as the first argument');
const {
  variant,
  instrumentation,
  measureEventLoop = false,
  _corpusDirectory: corpusDirectory,
  _entryPaths: entryPaths,
} = options;
if (variant !== 'ordinary' && !/^worker-(?:[1-9]|[1-5]\d|6[0-4])$/.test(variant)) {
  throw new Error(`invalid variant: ${variant}`);
}
if (typeof instrumentation !== 'boolean' || typeof measureEventLoop !== 'boolean') {
  throw new Error('instrumentation and measureEventLoop must be booleans');
}
if (typeof corpusDirectory !== 'string' || !Array.isArray(entryPaths) || entryPaths.length !== 56) {
  throw new Error('registry graph corpus metadata is incomplete');
}

const canonicalCorpusDirectory = await realpath(corpusDirectory);
const normalizeText = (value) =>
  value
    .replaceAll(canonicalCorpusDirectory, '<svelte-registry-corpus>')
    .replaceAll(corpusDirectory, '<svelte-registry-corpus>');
const normalizeLog = (level, log) => ({
  level,
  code: log.code,
  pluginCode: log.pluginCode,
  message: normalizeText(log.message),
  id: log.id ? normalizeText(log.id) : undefined,
  plugin: log.plugin,
  hook: log.hook,
  loc: log.loc
    ? { ...log.loc, file: log.loc.file ? normalizeText(log.loc.file) : undefined }
    : undefined,
  frame: log.frame,
});
const input = Object.fromEntries(
  entryPaths.map((path) => [nodePath.basename(nodePath.dirname(path)), path]),
);
if (Object.keys(input).length !== 56) throw new Error('registry graph entry names are not unique');
const telemetry = {
  aliasRequests: 0,
  aliasResolutions: 0,
  nodeNextResolutions: 0,
  relativeNodeNextRequests: 0,
  relativeNodeNextResolutions: 0,
};
const externalized = new Map();
const logs = [];
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
  const resolver = createRegistryGraphResolver({ corpusDirectory, telemetry });
  const pluginOptions = { corpusDirectory, metricsBuffer };
  const transformPlugin =
    variant === 'ordinary'
      ? (await import('../../svelte-registry-graph-plugin/kernel.js')).svelteRegistryGraphPlugin(
          pluginOptions,
        )
      : (await import('../../svelte-registry-graph-plugin/index.js')).default(pluginOptions);
  const pluginSetupFinishedAt = performance.now();

  const apiStartedAt = performance.now();
  build = await rolldown({
    cwd: corpusDirectory,
    input,
    logLevel: 'debug',
    treeshake: false,
    external(source, importer) {
      if (!isBareDependency(source)) return false;
      const key = `${source}\0${importer ?? ''}`;
      externalized.set(key, {
        source,
        importer: importer ? normalizeText(importer) : undefined,
      });
      return true;
    },
    onLog(level, log) {
      logs.push(normalizeLog(level, log));
    },
    plugins: [resolver, transformPlugin],
  });
  const generateStartedAt = performance.now();
  const result = await build.generate({
    format: 'esm',
    sourcemap: true,
    entryFileNames: '[name].js',
    chunkFileNames: 'chunks/[name]-[hash].js',
  });
  const generateFinishedAt = performance.now();
  await build.close();
  build = undefined;
  if (eventLoopMonitor) await new Promise((resolve) => setImmediate(resolve));
  const totalFinishedAt = performance.now();
  const cpu = process.cpuUsage(cpuStartedAt);

  const chunks = result.output
    .filter((output) => output.type === 'chunk')
    .sort((left, right) => left.fileName.localeCompare(right.fileName));
  const codeHash = createHash('sha256');
  const mapHash = createHash('sha256');
  let outputCodeBytes = 0;
  let outputMapBytes = 0;
  let totalExports = 0;
  let nullMapChunkCount = 0;
  const localModulePaths = new Set();
  for (const chunk of chunks) {
    const map =
      chunk.map === null
        ? undefined
        : typeof chunk.map === 'string'
          ? chunk.map
          : JSON.stringify(chunk.map);
    const code = normalizeText(chunk.code);
    outputCodeBytes += Buffer.byteLength(chunk.code);
    totalExports += chunk.exports.length;
    codeHash.update(chunk.fileName);
    codeHash.update('\0');
    codeHash.update(code);
    codeHash.update('\0');
    mapHash.update(chunk.fileName);
    mapHash.update('\0');
    if (map === undefined) {
      nullMapChunkCount++;
      mapHash.update('<no-map>');
    } else {
      outputMapBytes += Buffer.byteLength(map);
      mapHash.update(normalizeText(map));
    }
    mapHash.update('\0');
    for (const id of chunk.moduleIds) {
      if (
        id === canonicalCorpusDirectory ||
        id.startsWith(`${canonicalCorpusDirectory}${nodePath.sep}`)
      ) {
        localModulePaths.add(
          nodePath.relative(canonicalCorpusDirectory, id).split(nodePath.sep).join('/'),
        );
      }
    }
  }
  const sortedLocalModulePaths = [...localModulePaths].sort((left, right) =>
    Buffer.compare(Buffer.from(left), Buffer.from(right)),
  );
  const graphHash = createHash('sha256');
  let graphSourceBytes = 0;
  for (const path of sortedLocalModulePaths) {
    const content = await readFile(nodePath.join(canonicalCorpusDirectory, path));
    graphSourceBytes += content.byteLength;
    graphHash.update(path);
    graphHash.update('\0');
    graphHash.update(String(content.byteLength));
    graphHash.update('\0');
    graphHash.update(createHash('sha256').update(content).digest('hex'));
    graphHash.update('\n');
  }
  const externalizedImports = [...externalized.values()].sort(
    (left, right) =>
      left.source.localeCompare(right.source) ||
      (left.importer ?? '').localeCompare(right.importer ?? ''),
  );
  const projectLocalExternals = externalizedImports.filter(
    ({ source }) => !isBareDependency(source),
  );
  if (projectLocalExternals.length !== 0) throw new Error('project-local import was externalized');
  const bareExternalIds = [...new Set(externalizedImports.map(({ source }) => source))].sort(
    (left, right) => left.localeCompare(right),
  );
  const appVirtualExternals = bareExternalIds.filter((source) => source.startsWith('$app/'));
  const workspacePackageExternals = bareExternalIds.filter((source) =>
    source.startsWith('shadcn-svelte/'),
  );
  const svelteRuntimeExternals = bareExternalIds.filter(
    (source) => source === 'svelte' || source.startsWith('svelte/'),
  );
  const thirdPartyBareExternals = bareExternalIds.filter(
    (source) =>
      !appVirtualExternals.includes(source) &&
      !workspacePackageExternals.includes(source) &&
      !svelteRuntimeExternals.includes(source),
  );

  console.log(
    JSON.stringify({
      variant,
      instrumentation,
      measureEventLoop,
      totalElapsedMs: totalFinishedAt - totalStartedAt,
      pluginSetupElapsedMs: pluginSetupFinishedAt - totalStartedAt,
      rolldownApiElapsedMs: totalFinishedAt - apiStartedAt,
      generateElapsedMs: generateFinishedAt - generateStartedAt,
      closeElapsedMs: totalFinishedAt - generateFinishedAt,
      cpuUserMs: cpu.user / 1000,
      cpuSystemMs: cpu.system / 1000,
      finalRssBytes: process.memoryUsage.rss(),
      entryCount: entryPaths.length,
      localModuleCount: sortedLocalModulePaths.length,
      componentModuleCount: sortedLocalModulePaths.filter((path) => path.endsWith('.svelte'))
        .length,
      svelteModuleCount: sortedLocalModulePaths.filter((path) => /\.svelte\.(?:js|ts)$/.test(path))
        .length,
      typeScriptModuleCount: sortedLocalModulePaths.filter(
        (path) => path.endsWith('.ts') && !path.endsWith('.svelte.ts'),
      ).length,
      graphSourceBytes,
      graphHash: graphHash.digest('hex'),
      localModulePaths: sortedLocalModulePaths,
      outputChunkCount: chunks.length,
      outputAssetCount: result.output.length - chunks.length,
      outputCodeBytes,
      outputMapBytes,
      nullMapChunkCount,
      totalExports,
      outputCodeHash: codeHash.digest('hex'),
      outputMapHash: mapHash.digest('hex'),
      resolverTelemetry: telemetry,
      bareExternalIds,
      appVirtualExternals,
      workspacePackageExternals,
      svelteRuntimeExternals,
      thirdPartyBareExternals,
      bareExternalPackages: [
        ...new Set(
          externalizedImports.map(({ source }) => {
            const segments = source.split('/');
            return source.startsWith('@') ? segments.slice(0, 2).join('/') : segments[0];
          }),
        ),
      ].sort((left, right) => left.localeCompare(right)),
      externalizedImports,
      projectLocalExternalCount: projectLocalExternals.length,
      logs,
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
