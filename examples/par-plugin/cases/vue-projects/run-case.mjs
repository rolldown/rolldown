import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { rolldown } from 'rolldown';
import {
  assertFrozenProjectAdapterProvenance,
  captureProjectAdapterProvenance,
} from './adapter-provenance.mjs';
import {
  createAuditPlugins,
  createGraphSupport,
  createInputs,
  inspectGitLabCompilerContract,
} from './graph-support.mjs';
import { ensurePreparedProject } from './prepare-projects.mjs';
import { assertFormalPerformanceAuthorization } from './performance-policy.mjs';
import { assertLocalNode, projectDefinition, projectRoot } from './projects.mjs';

assertLocalNode();
const options = JSON.parse(process.argv[2] ?? 'null');
if (!options || typeof options !== 'object') throw new Error('expected a JSON case argument');
const { projectId, variant, collectPerformance = false } = options;
const project = projectDefinition(projectId);
if (variant !== 'ordinary' && !/^worker-[1-8]$/.test(variant)) {
  throw new Error(`invalid independent Vue project variant: ${variant}`);
}
if (typeof collectPerformance !== 'boolean') {
  throw new Error('collectPerformance must be a boolean');
}
assertFormalPerformanceAuthorization(collectPerformance, options.formalPerformanceProtocol);

const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const byteSort = (left, right) => Buffer.compare(Buffer.from(left), Buffer.from(right));
const normalizeError = (error, root) => {
  const stack = String(error?.stack ?? error).replaceAll(root, '<project-root>');
  return {
    name: error?.name ?? 'Error',
    message: String(error?.message ?? error).replaceAll(root, '<project-root>'),
    stackSha256: sha256(stack),
    stack,
  };
};

let build;
let prepared;
const caseStartedAt = collectPerformance ? performance.now() : undefined;
const caseCpuStartedAt = collectPerformance ? process.cpuUsage() : undefined;
caseExecution: try {
  prepared = collectPerformance
    ? assertFormalPreparedProject(options.formalPreparedProject, project)
    : await ensurePreparedProject(projectId);
  const root = prepared.root;
  process.chdir(root);
  const { input, virtualEntries, entryProvenance } = await createInputs(project, root);
  const adapterProvenance = options.frozenAdapterProvenance
    ? assertFrozenProjectAdapterProvenance(options.frozenAdapterProvenance, projectId)
    : await captureProjectAdapterProvenance(projectId, root);

  if (projectId === 'gitlab') {
    const compilerContract = await inspectGitLabCompilerContract(root, project.compilerContract);
    console.log(
      JSON.stringify({
        schema: 1,
        projectId,
        band: project.band,
        variant,
        measurementClass: 'correctness-only',
        executionStatus: 'not-run',
        admissionStatus: 'rejected',
        prepared,
        entryProvenance,
        adapterProvenance,
        compilerContract,
        rejection: {
          code: 'GITLAB_DUAL_COMPILER_CONTRACT_UNAVAILABLE',
          message:
            "The available transform-only adapter has one Vue 3 compiler instance and cannot preserve GitLab's Vue 2.7.16 plus Vue 3 compat 3.5.34 infection/compiler routing, custom-element settings, or loader lifecycle. Running it would produce a different graph, so direct Rolldown admission stops before transform.",
        },
        capabilityBoundary: {
          sourceMapSemanticCorrectness: {
            tested: false,
            reason: 'Transform does not run after the compiler-contract rejection.',
          },
          pluginDiagnosticParity: {
            tested: false,
            reason:
              'The retained rejection is a harness capability diagnostic, not an ordinary-versus-worker plugin diagnostic comparison.',
          },
        },
      }),
    );
    break caseExecution;
  }

  const graphSupport = await createGraphSupport(project, root, virtualEntries);
  const audit = createAuditPlugins(root);
  const pluginOptions = { root };
  const vuePlugin =
    variant === 'ordinary'
      ? (await import('../../parallel-vue-plugin/impl.js')).vueTransformPlugin(pluginOptions)
      : (await import('../../parallel-vue-plugin/index.js')).default(pluginOptions);
  const warnings = [];
  globalThis.gc?.();
  build = await rolldown({
    cwd: root,
    input,
    logLevel: 'silent',
    moduleTypes: { vue: 'js' },
    resolve: { symlinks: false },
    tsconfig: projectId === 'vben' ? false : true,
    treeshake: true,
    onLog(level, log) {
      warnings.push({
        level,
        code: log.code,
        message: String(log.message).replaceAll(root, '<project-root>'),
      });
    },
    plugins: [graphSupport.plugin, audit.sourceAudit, vuePlugin, audit.resultAudit],
  });
  const result = await build.generate({
    format: 'esm',
    sourcemap: true,
    entryFileNames: 'entries/[name]-[hash].js',
    chunkFileNames: 'chunks/[name]-[hash].js',
    assetFileNames: 'assets/[name]-[hash][extname]',
  });
  await build.close();
  build = undefined;

  const codeHash = createHash('sha256');
  const mapHash = createHash('sha256');
  let outputCodeBytes = 0;
  let outputMapBytes = 0;
  let outputChunkCount = 0;
  let outputAssetCount = 0;
  let outputEntryCount = 0;
  let outputDynamicEntryCount = 0;
  let outputMissingSourceMapCount = 0;
  let totalExports = 0;
  const outputFiles = [];
  for (const output of [...result.output].sort((left, right) =>
    byteSort(left.fileName, right.fileName),
  )) {
    outputFiles.push(output.fileName);
    if (output.type === 'asset') {
      outputAssetCount++;
      const source = String(output.source).replaceAll(root, '<project-root>');
      outputCodeBytes += Buffer.byteLength(source);
      codeHash.update(`asset\0${output.fileName}\0${source}\0`);
      continue;
    }
    outputChunkCount++;
    if (output.isEntry) outputEntryCount++;
    if (output.isDynamicEntry) outputDynamicEntryCount++;
    totalExports += output.exports.length;
    const code = output.code.replaceAll(root, '<project-root>');
    const map =
      typeof output.map === 'string'
        ? output.map
        : output.map
          ? JSON.stringify(output.map)
          : undefined;
    if (!map) outputMissingSourceMapCount++;
    const normalizedMap = map?.replaceAll(root, '<project-root>') ?? '<no-map>';
    outputCodeBytes += Buffer.byteLength(output.code);
    outputMapBytes += map ? Buffer.byteLength(map) : 0;
    codeHash.update(`chunk\0${output.fileName}\0${code}\0`);
    mapHash.update(`${output.fileName}\0${normalizedMap}\0`);
  }

  const transform = audit.report();
  const graph = graphSupport.report();
  const failures = [];
  if (!transform.exactOnce)
    failures.push('every reached SFC must enter source and result audit once');
  if (
    project.expectedReachedSfcCount !== undefined &&
    transform.reachedSfcCount !== project.expectedReachedSfcCount
  ) {
    failures.push(
      `reached ${transform.reachedSfcCount} SFCs instead of exact ${project.expectedReachedSfcCount}`,
    );
  }
  if (
    project.minimumReachedSfcCount !== undefined &&
    transform.reachedSfcCount < project.minimumReachedSfcCount
  ) {
    failures.push(
      `reached ${transform.reachedSfcCount} SFCs, below frozen minimum ${project.minimumReachedSfcCount}`,
    );
  }
  if (
    project.expectedObservedReachedSfcCount !== undefined &&
    transform.reachedSfcCount !== project.expectedObservedReachedSfcCount
  ) {
    failures.push(
      `reached ${transform.reachedSfcCount} SFCs instead of recorded ordinary admission count ${project.expectedObservedReachedSfcCount}`,
    );
  }
  if (project.knownUnreachedSfcPaths) {
    const reached = new Set(transform.reachedSfcPaths);
    const actualUnreached = prepared.physicalSfc.paths.filter((path) => !reached.has(path));
    if (JSON.stringify(actualUnreached) !== JSON.stringify(project.knownUnreachedSfcPaths)) {
      failures.push(
        `unreached SFC manifest drifted: expected ${project.knownUnreachedSfcPaths.length}, observed ${actualUnreached.length}`,
      );
    }
  }
  if (outputEntryCount !== entryProvenance.totalEntryCount) {
    failures.push(
      `generated ${outputEntryCount} entry chunks for ${entryProvenance.totalEntryCount} entry roots`,
    );
  }
  if (outputChunkCount !== 0 && outputMissingSourceMapCount === outputChunkCount) {
    failures.push('source maps were enabled but no output chunk retained a map');
  }
  const performanceCpu = collectPerformance ? process.cpuUsage(caseCpuStartedAt) : undefined;
  console.log(
    JSON.stringify({
      schema: 1,
      projectId,
      band: project.band,
      variant,
      measurementClass: collectPerformance ? 'formal-performance-child' : 'correctness-only',
      executionStatus: 'completed',
      admissionStatus: failures.length === 0 ? 'accepted' : 'rejected',
      admissionFailures: failures,
      prepared,
      entryProvenance,
      adapterProvenance,
      transform,
      graph,
      warnings,
      output: {
        codeSha256: codeHash.digest('hex'),
        sourceMapSha256: mapHash.digest('hex'),
        codeBytes: outputCodeBytes,
        sourceMapBytes: outputMapBytes,
        chunkCount: outputChunkCount,
        assetCount: outputAssetCount,
        entryCount: outputEntryCount,
        dynamicEntryCount: outputDynamicEntryCount,
        missingSourceMapCount: outputMissingSourceMapCount,
        totalExports,
        fileManifestSha256: sha256(`${outputFiles.join('\n')}\n`),
      },
      capabilityBoundary: {
        directRolldown: true,
        vite: false,
        transformAdapter: 'unplugin-vue transform and buildStart only',
        sourceMaps: 'Rolldown output maps; adapter transform sourceMap is disabled',
        sourceMapSemanticCorrectness: {
          tested: false,
          reason:
            'Only normalized output-map artifact parity is checked; the transform adapter disables its own source map, so source positions are not validated.',
        },
        pluginDiagnosticParity: {
          tested: false,
          reason:
            'This admission matrix contains no paired invalid-SFC or worker-failure diagnostic case.',
        },
        localStyleLoader: 'deterministic empty JS stub',
        localAssetLoader: 'deterministic URL/string stub',
        bareDependencies:
          'externalized; repository-local relative, alias, and workspace edges retained',
        tsconfig:
          projectId === 'vben'
            ? 'disabled because the pinned app extends the uninstalled @vben/tsconfig package; # and workspace source mappings are implemented by the graph adapter'
            : 'Rolldown discovery enabled',
      },
      performance: collectPerformance
        ? {
            elapsedMs: performance.now() - caseStartedAt,
            cpuUserMs: performanceCpu.user / 1000,
            cpuSystemMs: performanceCpu.system / 1000,
            finalRssBytes: process.memoryUsage.rss(),
          }
        : undefined,
    }),
  );
} catch (error) {
  console.log(
    JSON.stringify({
      schema: 1,
      projectId,
      band: project.band,
      variant,
      measurementClass: collectPerformance ? 'formal-performance-child' : 'correctness-only',
      executionStatus: 'failed',
      admissionStatus: 'rejected',
      prepared,
      error: normalizeError(error, prepared?.root ?? ''),
    }),
  );
  process.exitCode = 2;
} finally {
  await build?.close();
}

function assertFormalPreparedProject(value, definition) {
  if (
    !value ||
    value.projectId !== definition.id ||
    value.commit !== definition.commit ||
    value.root !== projectRoot(definition.id) ||
    value.physicalSfc?.count !== definition.expectedPhysicalSfcCount ||
    value.physicalSfc?.bytes !== definition.expectedPhysicalSfcBytes ||
    value.physicalSfc?.manifestSha256 !== definition.expectedPhysicalSfcManifestSha256 ||
    value.physicalSfc?.paths?.length !== definition.expectedPhysicalSfcCount
  ) {
    throw new Error('formal parent preparation snapshot differs from the frozen project');
  }
  if (
    definition.dependencyPreparation &&
    value.dependencyPreparation?.install?.packageManager !==
      definition.dependencyPreparation.packageManager
  ) {
    throw new Error('formal parent dependency snapshot differs from the frozen project');
  }
  return value;
}
