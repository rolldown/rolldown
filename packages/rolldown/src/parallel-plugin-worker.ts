import type {
  PerformanceObserver as NodePerformanceObserver,
  performance as NodePerformance,
} from 'node:perf_hooks';
import type { getHeapStatistics as getNodeHeapStatistics } from 'node:v8';
import { parentPort, workerData } from 'node:worker_threads';
import { registerPlugins } from './binding.cjs';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { defineParallelPluginImplementation } from './parallel-plugin';
import { bindingifyPlugin } from './plugin/bindingify-plugin';
import { PluginContextData } from './plugin/plugin-context-data';
import type {
  MetricsTimestamp,
  WorkerData,
  WorkerMetricsSnapshotRequest,
  WorkerMetricsSnapshotResponse,
} from './utils/initialize-parallel-plugins';

const { registryId, pluginInfos, threadNumber, metricsEnabled, metricsMainTimeOriginEpochMs } =
  workerData as WorkerData;
const workerEntryAt = metricsEnabled ? metricsTimestamp() : undefined;
(async () => {
  try {
    if (!metricsEnabled) {
      const plugins = await Promise.all(
        pluginInfos.map(async (pluginInfo) => {
          const pluginModule = await import(pluginInfo.fileUrl);
          const definePluginImpl = pluginModule.default as ReturnType<
            typeof defineParallelPluginImplementation
          >;
          const plugin = await definePluginImpl(pluginInfo.options, {
            threadNumber,
          });
          return {
            index: pluginInfo.index,
            // TODO(sapphi-red): support inputOptions and outputOptions
            plugin: bindingifyPlugin(
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
            ),
          };
        }),
      );
      registerPlugins(registryId, plugins);
      parentPort!.postMessage({ type: 'success' });
    } else {
      const bootstrapStartedAt = metricsTimestamp();
      const metricsRuntimeStartedAt = metricsTimestamp();
      const metricsRuntime = await createWorkerMetricsRuntime();
      const metricsRuntimeFinishedAt = metricsTimestamp();
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
              implementationImportMs: durationMs(importStartedAt, importFinishedAt),
              factoryMs: durationMs(factoryStartedAt, factoryFinishedAt),
              bindingifyMs: durationMs(bindingStartedAt, bindingFinishedAt),
              timeline: {
                importStartedAt,
                importFinishedAt,
                factoryStartedAt,
                factoryFinishedAt,
                bindingStartedAt,
                bindingFinishedAt,
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
      const readyAt = metricsTimestamp();

      installMetricsSnapshotListener(metricsRuntime);

      parentPort!.postMessage({
        type: 'success',
        metrics: {
          clockAlignment: {
            workerTimeOriginEpochMs: performance.timeOrigin,
            mainTimeOriginEpochMs: metricsMainTimeOriginEpochMs,
            workerMinusMainTimeOriginMs:
              metricsMainTimeOriginEpochMs === undefined
                ? undefined
                : performance.timeOrigin - metricsMainTimeOriginEpochMs,
          },
          timeline: {
            entryAt: workerEntryAt,
            bootstrapStartedAt,
            metricsRuntimeStartedAt,
            metricsRuntimeFinishedAt,
            registerStartedAt,
            registerFinishedAt,
            readyAt,
          },
          measuredBootstrapMs: durationMs(bootstrapStartedAt, registerFinishedAt),
          registerPluginsMs: durationMs(registerStartedAt, registerFinishedAt),
          plugins: initializedPlugins.map(({ metrics }) => metrics),
          workerLocalAtReady: captureWorkerLocalMetrics(metricsRuntime),
        },
      });
    }
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
    parentPort!.unref();
    return;
  }
  // Hold the worker alive so Rust can dispatch plugin hook callbacks through
  // the thread-safe functions registered during bootstrap. The main thread
  // terminates the worker explicitly when the build completes.
  setInterval(() => {}, 1 << 30);
})();

type WorkerMetricsRuntime = {
  performance: typeof NodePerformance;
  getHeapStatistics: typeof getNodeHeapStatistics;
  gcMetrics: ReturnType<typeof createGcMetricsCollector>;
};

function installMetricsSnapshotListener(metricsRuntime: WorkerMetricsRuntime) {
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

function captureWorkerLocalMetrics(metricsRuntime: WorkerMetricsRuntime) {
  return {
    capturedAt: metricsTimestamp(),
    scope: {
      heapStatistics: 'this worker V8 isolate',
      eventLoopUtilization: 'this worker event loop; this is not CPU time',
      gc: 'GC performance entries observed in this worker after entry instrumentation started',
    },
    heapStatistics: metricsRuntime.getHeapStatistics(),
    eventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
    gc: metricsRuntime.gcMetrics.snapshot(),
  };
}

function createGcMetricsCollector(PerformanceObserver: typeof NodePerformanceObserver) {
  const totals = new Map<number, { count: number; durationMs: number; maxDurationMs: number }>();
  let count = 0;
  let durationMs = 0;
  let maxDurationMs = 0;

  const collect = (
    entries: Array<{ duration: number; detail?: { kind?: number }; kind?: number }>,
  ) => {
    for (const entry of entries) {
      const kind = entry.detail?.kind ?? entry.kind ?? 0;
      const kindTotals = totals.get(kind) ?? { count: 0, durationMs: 0, maxDurationMs: 0 };
      kindTotals.count += 1;
      kindTotals.durationMs += entry.duration;
      kindTotals.maxDurationMs = Math.max(kindTotals.maxDurationMs, entry.duration);
      totals.set(kind, kindTotals);
      count += 1;
      durationMs += entry.duration;
      maxDurationMs = Math.max(maxDurationMs, entry.duration);
    }
  };

  const observer = new PerformanceObserver((list) => collect(list.getEntries()));
  observer.observe({ entryTypes: ['gc'] });

  return {
    snapshot: () => {
      collect(observer.takeRecords());
      return {
        count,
        durationMs,
        maxDurationMs,
        byKind: Object.fromEntries(
          [...totals.entries()].map(([kind, value]) => [String(kind), { kind, ...value }]),
        ),
      };
    },
  };
}

async function createWorkerMetricsRuntime(): Promise<WorkerMetricsRuntime> {
  const [{ performance, PerformanceObserver }, { getHeapStatistics }] = await Promise.all([
    import('node:perf_hooks'),
    import('node:v8'),
  ]);
  return {
    performance,
    getHeapStatistics,
    gcMetrics: createGcMetricsCollector(PerformanceObserver),
  };
}

function metricsTimestamp(): MetricsTimestamp {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
}

function durationMs(start: MetricsTimestamp, end: MetricsTimestamp) {
  return end.monotonicMs - start.monotonicMs;
}
