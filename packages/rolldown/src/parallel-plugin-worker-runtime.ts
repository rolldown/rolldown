import { parentPort, workerData } from 'node:worker_threads';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type {
  WorkerData,
  WorkerMetricsSnapshotRequest,
  WorkerMetricsSnapshotResponse,
} from './utils/initialize-parallel-plugins';
import {
  captureProcessMetrics,
  metricsStage,
  metricsTimestamp,
  type MetricsRuntime,
  type WorkerLauncherMetrics,
} from './utils/parallel-plugin-init-metrics';

type LauncherContext = {
  launcherMetrics: WorkerLauncherMetrics;
  metricsRuntime: MetricsRuntime;
};

export async function startParallelPluginWorker(launcherContext: LauncherContext) {
  const {
    registryId,
    pluginInfos,
    threadNumber,
    metricsEnabled,
    metricsId,
    metricsMainTimeOriginEpochMs,
  } = workerData as WorkerData;
  const runtimeEntryAt = metricsTimestamp();
  try {
    if (!metricsEnabled || metricsId === undefined) {
      throw new Error('parallel plugin worker metrics runtime context is missing');
    }
    const { launcherMetrics, metricsRuntime } = launcherContext;
    const bootstrapStartedAt = metricsTimestamp();
    const workerLocalBeforePluginInitialization = captureWorkerLocalMetrics(metricsRuntime);
    const initializedPlugins = await Promise.all(
      pluginInfos.map(async (pluginInfo) => {
        const importStartedAt = metricsTimestamp();
        const pluginModule = await import(pluginInfo.fileUrl);
        const importFinishedAt = metricsTimestamp();
        const definePluginImpl = pluginModule.default as ReturnType<
          typeof defineParallelPluginImplementation
        >;
        const factoryStartedAt = metricsTimestamp();
        const plugin = await definePluginImpl(pluginInfo.options, {
          threadNumber,
        });
        const factoryFinishedAt = metricsTimestamp();
        const bindingStartedAt = metricsTimestamp();
        const bindingPlugin = bindingifyPlugin(
          plugin,
          {} as InputOptions,
          {} as OutputOptions,
          // TODO need to find a way to share pluginContextData
          new PluginContextData(() => {}, {} as OutputOptions, [], []),
          [],
          () => {},
          'info' as const,
          // TODO: support this.meta.watchMode
          false,
        );
        const bindingFinishedAt = metricsTimestamp();
        return {
          registration: { index: pluginInfo.index, plugin: bindingPlugin },
          metrics: {
            pluginIndex: pluginInfo.index,
            implementationImportMs: importFinishedAt.monotonicMs - importStartedAt.monotonicMs,
            factoryMs: factoryFinishedAt.monotonicMs - factoryStartedAt.monotonicMs,
            bindingifyMs: bindingFinishedAt.monotonicMs - bindingStartedAt.monotonicMs,
            timeline: {
              importStartedAt,
              importFinishedAt,
              factoryStartedAt,
              factoryFinishedAt,
              bindingStartedAt,
              bindingFinishedAt,
            },
            stages: {
              implementationImport: metricsStage(importStartedAt, importFinishedAt),
              factory: metricsStage(factoryStartedAt, factoryFinishedAt),
              bindingifyPlugin: metricsStage(bindingStartedAt, bindingFinishedAt),
            },
          },
        };
      }),
    );

    const registerStartedAt = metricsTimestamp();
    registerPlugins(
      registryId,
      initializedPlugins.map(({ registration }) => registration),
    );
    const registerFinishedAt = metricsTimestamp();

    installMetricsSnapshotListener(metricsRuntime);
    const workerLocalAtReady = captureWorkerLocalMetrics(metricsRuntime);
    const readyAt = metricsTimestamp();

    parentPort!.postMessage({
      type: 'success',
      metrics: {
        kind: 'rolldown_parallel_plugin_worker_bootstrap_metrics',
        version: 1,
        metricsId,
        threadNumber,
        clockAlignment: {
          workerTimeOriginEpochMs: performance.timeOrigin,
          mainTimeOriginEpochMs: metricsMainTimeOriginEpochMs,
          workerMinusMainTimeOriginMs:
            metricsMainTimeOriginEpochMs === undefined
              ? undefined
              : performance.timeOrigin - metricsMainTimeOriginEpochMs,
        },
        launcher: launcherMetrics,
        timeline: {
          entryAt: launcherMetrics.timeline.launcherEntryAt,
          launcherEntryAt: launcherMetrics.timeline.launcherEntryAt,
          runtimeEntryAt,
          bootstrapStartedAt,
          runtimeAndBindingImportStartedAt:
            launcherMetrics.timeline.runtimeAndBindingImportStartedAt,
          runtimeAndBindingImportFinishedAt:
            launcherMetrics.timeline.runtimeAndBindingImportFinishedAt,
          registerStartedAt,
          registerFinishedAt,
          readyAt,
        },
        measuredBootstrapMs:
          registerFinishedAt.monotonicMs - launcherMetrics.timeline.launcherEntryAt.monotonicMs,
        registerPluginsMs: registerFinishedAt.monotonicMs - registerStartedAt.monotonicMs,
        plugins: initializedPlugins.map(({ metrics }) => metrics),
        workerLocalBeforePluginInitialization,
        workerLocalAtReady,
        isolationLimits: [
          'runtimeAndBindingImport is the dynamic import of the compiled worker-runtime graph; that graph statically imports binding.cjs, so JavaScript graph evaluation and native-addon loading cannot be separated without changing production module boundaries',
          'the GC observer starts after the lightweight launcher dynamically imports node:perf_hooks; GC before that observer exists cannot be recovered',
          'process RSS is shared by the main isolate, every worker isolate, native addon state, and runtime threads; it is not worker ownership',
        ],
      },
    });
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
    parentPort!.unref();
    return;
  }
  // Hold the worker alive so Rust can dispatch plugin hook callbacks through
  // the thread-safe functions registered during bootstrap. The main thread
  // terminates the worker explicitly when the build completes.
  setInterval(() => {}, 1 << 30);
}

function installMetricsSnapshotListener(metricsRuntime: MetricsRuntime) {
  parentPort!.on('message', (message: WorkerMetricsSnapshotRequest) => {
    if (message.type !== 'metrics-snapshot-request') {
      return;
    }
    parentPort!.postMessage({
      type: 'metrics-snapshot-response',
      requestId: message.requestId,
      metrics: captureWorkerLocalMetrics(metricsRuntime),
    } satisfies WorkerMetricsSnapshotResponse);
  });
}

function captureWorkerLocalMetrics(metricsRuntime: MetricsRuntime) {
  const processMetrics = captureProcessMetrics(metricsRuntime);
  return {
    capturedAt: processMetrics.capturedAt,
    scope: {
      cpuUsage: 'whole process; not this worker',
      threadCpuUsage: 'this Node.js worker thread',
      memoryUsage: 'whole process; RSS is not this worker',
      heapStatistics: 'this worker V8 isolate',
      eventLoopUtilization: 'this worker event loop; this is not CPU time',
      gc: 'GC performance entries observed in this worker after launcher instrumentation started',
    },
    processCpuUsageMicros: processMetrics.processCpuUsageMicros,
    threadCpuUsageMicros: processMetrics.mainThreadCpuUsageMicros,
    processMemoryUsageBytes: processMetrics.processMemoryUsageBytes,
    heapStatistics: processMetrics.isolateHeapStatistics,
    eventLoopUtilization: processMetrics.isolateEventLoopUtilization,
    gc: processMetrics.isolateGc,
  };
}
