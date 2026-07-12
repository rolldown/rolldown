import { createHash } from 'node:crypto';
import { readFile, realpath } from 'node:fs/promises';
import nodePath from 'node:path';
import { performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import {
  createMetricsBuffer,
  createTransformTimelineBuffer,
  readMetrics,
  readTransformTimeline,
} from '../../parallel-vue-plugin/metrics.js';
import { REQUIRED_NODE_VERSION } from './provenance.mjs';

if (process.version !== REQUIRED_NODE_VERSION) {
  throw new Error(
    `Vue scale case requires Node.js ${REQUIRED_NODE_VERSION}, got ${process.version}`,
  );
}

const options = JSON.parse(process.argv[2] ?? 'null');
if (!options) throw new Error('expected a JSON case as the first argument');
if (
  process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS === 'json' &&
  (!process.execArgv.includes('--expose-gc') || typeof globalThis.gc !== 'function')
) {
  throw new Error('instrumented Vue attribution requires Node.js --expose-gc');
}
const {
  variant,
  componentCount,
  instrumentation,
  auditSources,
  collectPerformance,
  _corpusDirectory: corpusDirectory,
  _resolvedCorpusDirectory: expectedResolvedCorpusDirectory,
  _entryPath: entryPath,
  _selectionPath: selectionPath,
  _selectedSourceBytes: selectedSourceBytes,
  _selectedInputSha256: selectedInputSha256,
  _sourceAuditExactOnceSha256: expectedSourceAuditExactOnceSha256,
  _selectionHash: selectionHash,
} = options;
if (variant !== 'ordinary' && !/^worker-[1-8]$/.test(variant)) {
  throw new Error(`invalid frozen Vue scale variant: ${variant}`);
}
for (const [name, value] of [
  ['instrumentation', instrumentation],
  ['auditSources', auditSources],
  ['collectPerformance', collectPerformance],
]) {
  if (typeof value !== 'boolean') throw new Error(`${name} must be a boolean`);
}
if (!Number.isSafeInteger(componentCount) || componentCount < 1) {
  throw new Error('componentCount must be a positive safe integer');
}
if (!Number.isSafeInteger(selectedSourceBytes) || selectedSourceBytes < 1) {
  throw new Error('_selectedSourceBytes must be a positive safe integer');
}
for (const [name, value] of [
  ['_corpusDirectory', corpusDirectory],
  ['_resolvedCorpusDirectory', expectedResolvedCorpusDirectory],
  ['_entryPath', entryPath],
  ['_selectionPath', selectionPath],
  ['_selectionHash', selectionHash],
  ['_selectedInputSha256', selectedInputSha256],
  ['_sourceAuditExactOnceSha256', expectedSourceAuditExactOnceSha256],
]) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${name} must be a non-empty string`);
  }
}

const selectedEntries = JSON.parse(await readFile(selectionPath, 'utf8'));
if (!Array.isArray(selectedEntries) || selectedEntries.length !== componentCount) {
  throw new Error('selection file count mismatch');
}
const resolvedCorpusDirectory = await realpath(corpusDirectory);
if (resolvedCorpusDirectory !== expectedResolvedCorpusDirectory) {
  throw new Error('resolved corpus directory changed between parent and child');
}
const resolvedEntryPath = await realpath(entryPath);
const expectedIds = new Map(
  selectedEntries.map((entry) => [nodePath.join(resolvedCorpusDirectory, entry.sourceKey), entry]),
);
const auditCounts = new Map();
const auditContent = new Map();
let auditInputBytes = 0;
const sourceAuditPlugin = {
  name: 'vue-scale-source-audit',
  transform: {
    filter: { id: { include: [/\.vue$/] } },
    handler(code, id) {
      const expected = expectedIds.get(id);
      if (!expected) throw new Error(`unexpected Vue source reached controlled curve: ${id}`);
      auditCounts.set(id, (auditCounts.get(id) ?? 0) + 1);
      auditContent.set(id, {
        bytes: Buffer.byteLength(code),
        sha256: createHash('sha256').update(code).digest('hex'),
      });
      auditInputBytes += Buffer.byteLength(code);
      return null;
    },
  },
};
const vueExportHelperId = '\0/plugin-vue/export-helper';
const vueExportHelperPlugin = {
  name: 'vue-scale-export-helper',
  resolveId(id) {
    if (id === vueExportHelperId) return id;
  },
  load(id) {
    if (id === vueExportHelperId) {
      return `export default (sfc, props) => {
  const target = sfc.__vccOpts || sfc;
  for (const [key, val] of props) target[key] = val;
  return target;
}`;
    }
  },
};

const replacements = [
  [resolvedCorpusDirectory, '<vue-scale-corpus>'],
  [corpusDirectory, '<vue-scale-corpus>'],
  [nodePath.dirname(resolvedEntryPath), '<vue-scale-case>'],
  [nodePath.dirname(entryPath), '<vue-scale-case>'],
].sort((left, right) => right[0].length - left[0].length);
const normalize = (value) => {
  let normalized = value;
  for (const [path, replacement] of replacements)
    normalized = normalized.replaceAll(path, replacement);
  return normalized;
};

const captureClockAnchor = () => {
  const epochBeforeMs = Date.now();
  const hrtimeNs = process.hrtime.bigint();
  const epochAfterMs = Date.now();
  return {
    hrtimeNs: hrtimeNs.toString(),
    epochBeforeMs,
    epochAfterMs,
    epochBracketWidthMs: epochAfterMs - epochBeforeMs,
    epochEstimateMs: (epochBeforeMs + epochAfterMs) / 2,
    estimateUncertaintyMs: (epochAfterMs - epochBeforeMs) / 2,
  };
};

let build;
try {
  globalThis.gc?.();
  const clockAnchorBeforePlugin = instrumentation ? captureClockAnchor() : undefined;
  const cpuStartedAt = collectPerformance ? process.cpuUsage() : undefined;
  const totalStartedAt = collectPerformance ? performance.now() : undefined;
  const metricsBuffer = instrumentation ? createMetricsBuffer() : undefined;
  const timelineBuffer = instrumentation
    ? createTransformTimelineBuffer(componentCount)
    : undefined;
  const sourceOrdinals = instrumentation
    ? Object.fromEntries(
        selectedEntries.map((entry, ordinal) => [
          nodePath.join(resolvedCorpusDirectory, entry.sourceKey),
          ordinal,
        ]),
      )
    : undefined;
  const pluginOptions = {
    root: resolvedCorpusDirectory,
    metricsBuffer,
    timelineBuffer,
    sourceOrdinals,
  };
  const plugin =
    variant === 'ordinary'
      ? (await import('../../parallel-vue-plugin/impl.js')).vueTransformPlugin(pluginOptions)
      : (await import('../../parallel-vue-plugin/index.js')).default(pluginOptions);
  const pluginSetupFinishedAt = collectPerformance ? performance.now() : undefined;

  const apiStartedAt = collectPerformance ? performance.now() : undefined;
  build = await rolldown({
    cwd: resolvedCorpusDirectory,
    input: resolvedEntryPath,
    logLevel: 'silent',
    moduleTypes: { vue: 'js' },
    resolve: { symlinks: false },
    treeshake: false,
    external: (_source, importer) =>
      Boolean(importer && nodePath.resolve(importer) !== resolvedEntryPath),
    plugins: auditSources
      ? [sourceAuditPlugin, vueExportHelperPlugin, plugin]
      : [vueExportHelperPlugin, plugin],
  });
  const generateStartedAt = collectPerformance ? performance.now() : undefined;
  const result = await build.generate({ format: 'esm', sourcemap: true });
  const generateFinishedAt = collectPerformance ? performance.now() : undefined;
  await build.close();
  build = undefined;
  const clockAnchorAfterBuild = instrumentation ? captureClockAnchor() : undefined;
  const totalFinishedAt = collectPerformance ? performance.now() : undefined;
  const cpu = collectPerformance ? process.cpuUsage(cpuStartedAt) : undefined;

  const rawCodeHash = createHash('sha256');
  const codeHash = createHash('sha256');
  const rawMapHash = createHash('sha256');
  const mapHash = createHash('sha256');
  let outputCodeBytes = 0;
  let outputMapBytes = 0;
  let outputChunkCount = 0;
  let outputAssetCount = 0;
  let totalExports = 0;
  for (const output of [...result.output].sort((left, right) =>
    left.fileName.localeCompare(right.fileName),
  )) {
    if (output.type === 'asset') {
      outputAssetCount++;
      const source = String(output.source);
      rawCodeHash.update(`asset\0${output.fileName}\0${source}\0`);
      codeHash.update(`asset\0${output.fileName}\0${normalize(source)}\0`);
      outputCodeBytes += Buffer.byteLength(source);
      continue;
    }
    outputChunkCount++;
    totalExports += output.exports.length;
    const map =
      typeof output.map === 'string'
        ? output.map
        : output.map
          ? JSON.stringify(output.map)
          : undefined;
    if (!map) throw new Error(`missing generated source map for ${output.fileName}`);
    outputCodeBytes += Buffer.byteLength(output.code);
    outputMapBytes += Buffer.byteLength(map);
    for (const [hash, value] of [
      [rawCodeHash, output.code],
      [codeHash, normalize(output.code)],
      [rawMapHash, map],
      [mapHash, normalize(map)],
    ]) {
      hash.update(output.fileName);
      hash.update('\0');
      hash.update(value);
      hash.update('\0');
    }
  }

  let sourceAudit;
  if (auditSources) {
    const auditHash = createHash('sha256');
    const inputHash = createHash('sha256');
    for (const [id, entry] of expectedIds) {
      const count = auditCounts.get(id) ?? 0;
      if (count !== 1)
        throw new Error(`expected one transform arrival for ${entry.sourceKey}, got ${count}`);
      auditHash.update(entry.sourceKey);
      auditHash.update('\0');
      auditHash.update(String(count));
      auditHash.update('\n');
      const content = auditContent.get(id);
      if (!content) throw new Error(`missing audited input for ${entry.sourceKey}`);
      inputHash.update(entry.sourceKey);
      inputHash.update('\0');
      inputHash.update(String(content.bytes));
      inputHash.update('\0');
      inputHash.update(content.sha256);
      inputHash.update('\n');
    }
    if (auditCounts.size !== componentCount || auditInputBytes !== selectedSourceBytes) {
      throw new Error('Vue source audit count or byte total mismatch');
    }
    sourceAudit = {
      distinctIds: auditCounts.size,
      calls: [...auditCounts.values()].reduce((total, count) => total + count, 0),
      inputBytes: auditInputBytes,
      exactOnceSha256: auditHash.digest('hex'),
      inputAggregateSha256: inputHash.digest('hex'),
    };
    if (
      sourceAudit.exactOnceSha256 !== expectedSourceAuditExactOnceSha256 ||
      sourceAudit.inputAggregateSha256 !== selectedInputSha256
    ) {
      throw new Error('Vue source audit hashes differ from the parent selection');
    }
  }

  console.log(
    JSON.stringify({
      variant,
      componentCount,
      instrumentation,
      auditSources,
      measurementClass: collectPerformance ? 'performance-or-attribution' : 'correctness-only',
      selectionHash,
      expectedMatchingHandlerCalls: componentCount,
      selectedSourceBytes,
      totalElapsedMs: collectPerformance ? totalFinishedAt - totalStartedAt : undefined,
      pluginSetupElapsedMs: collectPerformance ? pluginSetupFinishedAt - totalStartedAt : undefined,
      rolldownApiElapsedMs: collectPerformance ? totalFinishedAt - apiStartedAt : undefined,
      generateElapsedMs: collectPerformance ? generateFinishedAt - generateStartedAt : undefined,
      closeElapsedMs: collectPerformance ? totalFinishedAt - generateFinishedAt : undefined,
      cpuUserMs: collectPerformance ? cpu.user / 1000 : undefined,
      cpuSystemMs: collectPerformance ? cpu.system / 1000 : undefined,
      finalRssBytes: collectPerformance ? process.memoryUsage.rss() : undefined,
      outputCodeBytes,
      outputMapBytes,
      outputChunkCount,
      outputAssetCount,
      totalExports,
      outputRawCodeHash: rawCodeHash.digest('hex'),
      outputCodeHash: codeHash.digest('hex'),
      outputRawMapHash: rawMapHash.digest('hex'),
      outputMapHash: mapHash.digest('hex'),
      sourceAudit,
      jsMetrics: readMetrics(metricsBuffer),
      clockAnchors: instrumentation
        ? {
            clock: 'process.hrtime.bigint() bracketed by Date.now()',
            beforePlugin: clockAnchorBeforePlugin,
            afterBuild: clockAnchorAfterBuild,
          }
        : undefined,
      transformTimeline: readTransformTimeline(
        timelineBuffer,
        selectedEntries.map((entry) => entry.sourceKey),
      ),
    }),
  );
} finally {
  await build?.close();
}
