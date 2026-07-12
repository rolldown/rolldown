import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import nodePath from 'node:path';
import {
  validateCreateBundlerOptionsMetrics,
  validateNativePluginRegistrationMetrics,
  validateParallelPluginLifecycleMetrics,
  validateWorkerBootstrapMetrics,
  validateWorkerLauncherMetrics,
} from '../../packages/rolldown/src/utils/parallel-plugin-init-metrics.ts';

if (process.version !== 'v24.18.0') {
  throw new Error(
    `initialization metrics contract requires Node.js v24.18.0, got ${process.version}`,
  );
}

const childIndex = process.argv.indexOf('--child');
if (childIndex !== -1) {
  await runChild(process.argv[childIndex + 1]);
} else {
  verifyMetricsOffSourceContract();
  verifySyntheticContracts();
  verifyBuiltRuntimeContract();
  verifyPostCreationCleanupContract();
  process.stdout.write('initialization metrics contract: ok\n');
}

function verifyMetricsOffSourceContract() {
  const initializerSource = readFileSync(
    new URL('../../packages/rolldown/src/utils/initialize-parallel-plugins.ts', import.meta.url),
    'utf8',
  );
  const plainWorkerSource = readFileSync(
    new URL('../../packages/rolldown/src/parallel-plugin-worker.ts', import.meta.url),
    'utf8',
  );
  const contracts = [
    {
      label: 'metrics-off initializeWorkers from attribution base 8e35a2249',
      source: sourceSlice(
        initializerSource,
        'async function initializeWorkers(',
        'async function initializeWorkersWithMetrics(',
      ),
      sha256: 'daa5d3d4c62911d2391270084510517b9ef40efc95e6bb0837f17645b1d1c19d',
    },
    {
      label: 'metrics-off initializeWorker from attribution base 8e35a2249',
      source: sourceSlice(
        initializerSource,
        'async function initializeWorker(',
        'async function initializeWorkerWithMetrics(',
      ),
      sha256: 'fe8f6204fc3f2f610e8b45346629a4b294f79ca7b4e4041fda09c441920ea7e3',
    },
    {
      label: 'original static metrics-off worker entry',
      source: plainWorkerSource,
      sha256: 'e8e31cbf8de2bd1948e0f37510e5b29a75657ce3979c6ffe20e87999b4a50f2a',
    },
  ];
  for (const contract of contracts) {
    assert.equal(
      createHash('sha256').update(contract.source).digest('hex'),
      contract.sha256,
      `${contract.label} changed`,
    );
  }
}

function sourceSlice(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  assert.ok(start >= 0 && end > start, `source contract markers missing: ${startMarker}`);
  return source.slice(start, end);
}

async function runChild(variant) {
  if (!['ordinary', 'parallel'].includes(variant)) throw new Error(`invalid variant ${variant}`);
  const caseRoot = nodePath.join(import.meta.dirname, 'init-metrics-contract');
  const [{ rolldown }, { defineParallelPlugin }] = await Promise.all([
    import('../../packages/rolldown/dist/index.mjs'),
    import('../../packages/rolldown/dist/experimental-index.mjs'),
  ]);
  const metricsBuffer = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * 5);
  const counters = new Int32Array(metricsBuffer);
  const plugin =
    variant === 'parallel'
      ? defineParallelPlugin(nodePath.join(caseRoot, 'parallel-impl.mjs'))({ metricsBuffer })
      : (() => {
          Atomics.add(counters, 0, 1);
          return {
            name: 'initialization-contract',
            buildStart() {
              Atomics.add(counters, 1, 1);
            },
            transform: {
              filter: { id: /input\.js$/ },
              handler(code) {
                Atomics.add(counters, 2, 1);
                return `${code}\n/* initialization-contract */`;
              },
            },
            buildEnd() {
              Atomics.add(counters, 3, 1);
            },
            closeBundle() {
              Atomics.add(counters, 4, 1);
            },
          };
        })();
  let build;
  try {
    build = await rolldown({
      cwd: caseRoot,
      input: 'input.js',
      logLevel: 'silent',
      plugins: [plugin],
    });
  } catch (error) {
    if (process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT) {
      process.stdout.write(
        `${JSON.stringify({ cleanupFault: process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT, error: String(error) })}\n`,
      );
      return;
    }
    throw error;
  }
  let result;
  try {
    result = await build.generate({ format: 'esm' });
  } catch (error) {
    if (process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT) {
      await build.close();
      process.stdout.write(
        `${JSON.stringify({ cleanupFault: process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT, error: String(error) })}\n`,
      );
      return;
    }
    throw error;
  } finally {
    if (!process.env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT) await build.close();
  }
  {
    const chunks = result.output.filter(({ type }) => type === 'chunk');
    const hash = createHash('sha256');
    for (const chunk of chunks) {
      hash.update(chunk.fileName);
      hash.update('\0');
      hash.update(chunk.code);
      hash.update('\0');
    }
    process.stdout.write(
      `${JSON.stringify({
        variant,
        chunks: chunks.length,
        outputSha256: hash.digest('hex'),
        lifecycleCounters: {
          factory: Atomics.load(counters, 0),
          buildStart: Atomics.load(counters, 1),
          transform: Atomics.load(counters, 2),
          buildEnd: Atomics.load(counters, 3),
          closeBundle: Atomics.load(counters, 4),
        },
      })}\n`,
    );
  }
}

function verifyBuiltRuntimeContract() {
  const reports = new Map();
  for (const variant of ['ordinary', 'parallel']) {
    for (const metrics of [false, true]) {
      const env = { ...process.env };
      delete env.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
      delete env.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
      delete env.ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT;
      if (variant === 'parallel') env.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = '2';
      if (metrics) {
        env.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
      }
      const result = spawnSync(
        process.execPath,
        ['--expose-gc', import.meta.filename, '--child', variant],
        {
          cwd: nodePath.resolve(import.meta.dirname, '../..'),
          env,
          encoding: 'utf8',
          timeout: 30_000,
        },
      );
      assert.equal(
        result.status,
        0,
        `${variant}/${metrics ? 'metrics' : 'plain'}: ${result.stderr}`,
      );
      const output = JSON.parse(result.stdout.trim());
      reports.set(`${variant}/${metrics}`, output);
      const create = parseRecords(result.stderr, 'rolldown-create-bundler-options-metrics');
      const native = parseRecords(result.stderr, 'rolldown-native-plugin-registration-metrics');
      const lifecycle = parseRecords(result.stderr, 'rolldown-parallel-plugin-init-metrics');
      if (!metrics) {
        assert.deepEqual(create, []);
        assert.deepEqual(native, []);
        assert.deepEqual(lifecycle, []);
        continue;
      }
      assert.equal(create.length, 1);
      assert.equal(native.length, 1);
      validateCreateBundlerOptionsMetrics(create[0]);
      validateNativePluginRegistrationMetrics(native[0]);
      assert.equal(native[0].metricsId, create[0].metricsId);
      assert.deepEqual(
        native[0].plugins.map(({ index, kind }) => ({ index, kind })),
        create[0].pluginBinding.map(({ pluginIndex, pluginKind }) => ({
          index: pluginIndex,
          kind: pluginKind === 'parallel-placeholder' ? 'parallel-js' : pluginKind,
        })),
      );
      assert.deepEqual(
        {
          ordinaryJs: native[0].ordinaryJsPluginCount,
          parallelPlaceholders: native[0].parallelJsPluginCount,
          builtin: native[0].builtinPluginCount,
        },
        {
          ordinaryJs: create[0].pluginCounts.ordinaryJs,
          parallelPlaceholders: create[0].pluginCounts.parallelPlaceholders,
          builtin: create[0].pluginCounts.builtin,
        },
      );
      const invalidCreate = structuredClone(create[0]);
      invalidCreate.pluginCounts.ordinaryJs += 1;
      assert.throws(
        () => validateCreateBundlerOptionsMetrics(invalidCreate),
        /counts do not match/,
      );
      const shiftedCreateResourceClock = structuredClone(create[0]);
      shiftedCreateResourceClock.resources.afterPluginNormalization.capturedAt.epochMs += 1;
      assert.throws(
        () => validateCreateBundlerOptionsMetrics(shiftedCreateResourceClock),
        /clock origin/,
      );
      const outOfOrderCreateResource = structuredClone(create[0]);
      outOfOrderCreateResource.resources.atCreateBundlerOptionsFinish.capturedAt = structuredClone(
        outOfOrderCreateResource.timeline.createBundlerOptionsStartedAt,
      );
      assert.throws(
        () => validateCreateBundlerOptionsMetrics(outOfOrderCreateResource),
        /resource containment\/order/,
      );
      const shiftedCreateStageClock = structuredClone(create[0]);
      shiftedCreateStageClock.stages.outputOptionsHook.startedAt.epochMs += 1;
      assert.throws(
        () => validateCreateBundlerOptionsMetrics(shiftedCreateStageClock),
        /clock origin/,
      );
      const invalidNative = structuredClone(native[0]);
      invalidNative.ordinaryJsPluginCount += 1;
      assert.throws(
        () => validateNativePluginRegistrationMetrics(invalidNative),
        /counts do not match/,
      );
      const invalidNativeStage = structuredClone(native[0]);
      invalidNativeStage.stages.pluginMaterializationMs =
        invalidNativeStage.stages.bindingOptionNormalizationMs + 1;
      assert.throws(
        () => validateNativePluginRegistrationMetrics(invalidNativeStage),
        /stage containment/,
      );
      if (variant === 'ordinary') {
        assert.equal(create[0].pluginCounts.ordinaryJs, 1);
        assert.equal(create[0].pluginCounts.parallelPlaceholders, 0);
        assert.deepEqual(lifecycle, []);
      } else {
        assert.equal(create[0].pluginCounts.ordinaryJs, 0);
        assert.equal(create[0].pluginCounts.parallelPlaceholders, 1);
        assert.equal(lifecycle.length, 2);
        for (const record of lifecycle) validateParallelPluginLifecycleMetrics(record);
        const initialization = lifecycle.find(
          ({ kind }) => kind === 'rolldown_parallel_plugin_init_metrics',
        );
        const termination = lifecycle.find(
          ({ kind }) => kind === 'rolldown_parallel_plugin_termination_metrics',
        );
        const parallelPluginIndexes = create[0].pluginBinding
          .filter(({ pluginKind }) => pluginKind === 'parallel-placeholder')
          .map(({ pluginIndex }) => pluginIndex);
        assert.equal(initialization.metricsId, create[0].metricsId);
        assert.equal(termination.metricsId, create[0].metricsId);
        assert.deepEqual(initialization.parallelPluginIndexes, parallelPluginIndexes);
        assert.deepEqual(termination.parallelPluginIndexes, parallelPluginIndexes);
        assert.equal(initialization.workers.length, 2);
        for (const worker of initialization.workers) {
          validateWorkerBootstrapMetrics(
            worker.workerBootstrap,
            worker.threadNumber,
            create[0].metricsId,
            parallelPluginIndexes,
          );
          validateWorkerLauncherMetrics(worker.workerBootstrap.launcher);
          assert.equal(worker.workerBootstrap.metricsId, create[0].metricsId);
          assert.equal(worker.workerBootstrap.launcher.metricsId, create[0].metricsId);
        }
        const invalidLifecycle = structuredClone(initialization);
        invalidLifecycle.processSnapshots.allWorkersReady.mainIsolateGc = undefined;
        assert.throws(() => validateParallelPluginLifecycleMetrics(invalidLifecycle), /main GC/);
        const shiftedLifecycleClock = structuredClone(initialization);
        shiftedLifecycleClock.processSnapshots.allWorkersReady.capturedAt.epochMs += 1;
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(shiftedLifecycleClock),
          /clock origin/,
        );
        const reversedResourceCapture = structuredClone(initialization);
        reversedResourceCapture.workers[0].resourcesAtPoolReady.snapshot.captureFinishedAt =
          structuredClone(
            reversedResourceCapture.workers[0].resourcesAtPoolReady.snapshot.captureStartedAt,
          );
        reversedResourceCapture.workers[0].resourcesAtPoolReady.snapshot.captureFinishedAt.monotonicMs -= 1;
        reversedResourceCapture.workers[0].resourcesAtPoolReady.snapshot.captureFinishedAt.epochMs -= 1;
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(reversedResourceCapture),
          /timeline regresses/,
        );
        const shiftedWorkerLocalClock = structuredClone(initialization);
        shiftedWorkerLocalClock.workers[0].workerBootstrap.workerLocalAtReady.capturedAt.epochMs += 1;
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(shiftedWorkerLocalClock),
          /clock origin/,
        );
        const outOfBoundWorkerLocal = structuredClone(initialization);
        outOfBoundWorkerLocal.workers[0].workerBootstrap.workerLocalBeforePluginInitialization.capturedAt =
          structuredClone(
            outOfBoundWorkerLocal.workers[0].workerBootstrap.timeline.launcherEntryAt,
          );
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(outOfBoundWorkerLocal),
          /outside the worker bootstrap timeline/,
        );
        const invalidReadyOrder = structuredClone(initialization);
        invalidReadyOrder.workers[0].mainTimeline.onlineAt =
          invalidReadyOrder.workers[0].mainTimeline.readyMessageAt;
        invalidReadyOrder.workers[0].mainTimeline.readyMessageAt =
          invalidReadyOrder.workers[0].mainTimeline.constructorReturnedAt;
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(invalidReadyOrder),
          /constructor, online, and ready order/,
        );
        const falselyExactCpu = structuredClone(initialization);
        falselyExactCpu.cpuWindows.measurementClass = 'exact attribution';
        assert.throws(
          () => validateParallelPluginLifecycleMetrics(falselyExactCpu),
          /CPU window diagnostic header/,
        );
      }
    }
  }
  assert.deepEqual(reports.get('ordinary/false'), reports.get('ordinary/true'));
  assert.deepEqual(reports.get('parallel/false'), reports.get('parallel/true'));
  assert.deepEqual(reports.get('ordinary/true').lifecycleCounters, {
    factory: 1,
    buildStart: 1,
    transform: 1,
    buildEnd: 1,
    closeBundle: 1,
  });
  assert.deepEqual(reports.get('parallel/true').lifecycleCounters, {
    factory: 2,
    buildStart: 2,
    transform: 1,
    buildEnd: 2,
    closeBundle: 0,
  });
  assert.equal(
    reports.get('ordinary/true').outputSha256,
    reports.get('parallel/true').outputSha256,
  );
}

function verifyPostCreationCleanupContract() {
  for (const fault of ['pool-after-worker-creation', 'create-after-pool-initialization']) {
    const result = spawnSync(
      process.execPath,
      ['--expose-gc', import.meta.filename, '--child', 'parallel'],
      {
        cwd: nodePath.resolve(import.meta.dirname, '../..'),
        env: {
          ...process.env,
          ROLLDOWN_PARALLEL_PLUGIN_METRICS: 'json',
          ROLLDOWN_PARALLEL_PLUGIN_WORKERS: '2',
          ROLLDOWN_PARALLEL_PLUGIN_METRICS_FAULT: fault,
        },
        encoding: 'utf8',
        timeout: 10_000,
      },
    );
    assert.equal(result.error, undefined, `${fault}: child did not exit after cleanup`);
    assert.equal(result.status, 0, `${fault}: ${result.stderr}`);
    const output = JSON.parse(result.stdout.trim());
    assert.equal(output.cleanupFault, fault);
    assert.match(output.error, /injected metrics fault/);
  }
}

function parseRecords(stderr, prefix) {
  const expression = new RegExp(`^\\[${prefix}\\] (\\{.*\\})$`, 'gm');
  return [...stderr.matchAll(expression)].map((match) => JSON.parse(match[1]));
}

function verifySyntheticContracts() {
  const launcher = makeLauncher();
  validateWorkerLauncherMetrics(launcher);
  const regressed = structuredClone(launcher);
  regressed.timeline.runtimeAndBindingImportStartedAt = timestamp(2);
  assert.throws(() => validateWorkerLauncherMetrics(regressed), /regresses/);
  const uncorrelatedClock = structuredClone(launcher);
  uncorrelatedClock.timeline.runtimeAndBindingImportStartedAt.epochMs += 1;
  assert.throws(() => validateWorkerLauncherMetrics(uncorrelatedClock), /clock origin/);

  validateNativePluginRegistrationMetrics(makeNativeMetrics());
  assert.throws(
    () =>
      validateNativePluginRegistrationMetrics(
        makeNativeMetrics({
          ordinaryJsPluginCount: 2,
          plugins: [
            { index: 0, name: 'a', kind: 'ordinary-js', materializationMs: 0.5 },
            { index: 0, name: 'b', kind: 'ordinary-js', materializationMs: 0.5 },
          ],
        }),
      ),
    /entry/,
  );
}

function makeNativeMetrics(overrides = {}) {
  return {
    kind: 'rolldown_native_plugin_registration_metrics',
    version: 1,
    metricsId: 1,
    boundary:
      'after BindingBundlerOptions destructuring, before registry transfer, through BundlerConfig construction, synchronously before ClassicBundler::create_bundle and Bundle::scan',
    nativeNormalizationTotalMs: 4,
    nativePluginMaterializationMs: 1,
    stages: {
      registryTransferMs: 0.5,
      workerManagerConstructionMs: 0.5,
      bindingOptionNormalizationMs: 2,
      pluginMaterializationMs: 1,
    },
    stageRelationships: {
      registryTransfer: 'direct child',
      workerManagerConstruction: 'direct child',
      bindingOptionNormalization: 'direct child',
      pluginMaterialization: 'nested child',
    },
    parallelRegistryPresent: false,
    workerManagerWorkerCount: 0,
    ordinaryJsPluginCount: 1,
    parallelJsPluginCount: 0,
    builtinPluginCount: 0,
    plugins: [{ index: 0, name: 'ordinary', kind: 'ordinary-js', materializationMs: 0.5 }],
    scope: 'test scope',
    ...overrides,
  };
}

function makeLauncher() {
  return {
    kind: 'rolldown_parallel_plugin_worker_launcher_metrics',
    version: 1,
    metricsId: 1,
    scope: 'test scope',
    timeline: {
      launcherEntryAt: timestamp(1),
      metricsRuntimeImportStartedAt: timestamp(2),
      metricsRuntimeImportFinishedAt: timestamp(3),
      runtimeAndBindingImportStartedAt: timestamp(4),
      runtimeAndBindingImportFinishedAt: timestamp(5),
    },
    stages: {
      metricsRuntimeImport: stage(2, 3),
      runtimeAndBindingImport: stage(4, 5),
    },
    resources: {
      afterMetricsRuntimeImportBeforeRuntimeAndBindingImport: processSnapshot(3.5),
      afterRuntimeAndBindingImport: processSnapshot(5.5),
    },
  };
}

function timestamp(monotonicMs) {
  return { monotonicMs, epochMs: 1_000 + monotonicMs };
}

function stage(start, finish) {
  return { startedAt: timestamp(start), finishedAt: timestamp(finish), durationMs: finish - start };
}

function processSnapshot(at) {
  return {
    capturedAt: timestamp(at),
    scope: {},
    processCpuUsageMicros: { user: 10, system: 1 },
    mainThreadCpuUsageMicros: { user: 5, system: 1 },
    processResourceUsage: {},
    processMemoryUsageBytes: { rss: 1_000 },
    isolateHeapStatistics: { heap_size_limit: 10_000 },
    isolateEventLoopUtilization: {},
    isolateGc: { count: 0, durationMs: 0, maxDurationMs: 0, byKind: {} },
  };
}
