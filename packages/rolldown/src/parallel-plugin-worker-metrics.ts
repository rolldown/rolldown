import type { PerformanceObserver as NodePerformanceObserver } from 'node:perf_hooks';
import { parentPort, workerData } from 'node:worker_threads';
import type { WorkerData } from './utils/initialize-parallel-plugins';
import type {
  GcMetricsSnapshot,
  MetricsRuntime,
  WorkerLauncherMetrics,
} from './utils/parallel-plugin-init-metrics';

// This research-only entry is selected only when metrics are enabled. The
// production metrics-off entry remains the original static worker graph.
const launcherEntryAt = timestamp();

void (async () => {
  try {
    const { metricsEnabled, metricsId } = workerData as WorkerData;
    if (!metricsEnabled || metricsId === undefined) {
      throw new Error('parallel plugin metrics worker was started without a metrics identity');
    }

    const metricsRuntimeImportStartedAt = timestamp();
    const metricsRuntime = await createLauncherMetricsRuntime();
    const metricsRuntimeImportFinishedAt = timestamp();
    const afterMetricsRuntimeImportBeforeRuntimeAndBindingImport =
      captureLauncherProcessMetrics(metricsRuntime);
    const runtimeAndBindingImportStartedAt = timestamp();
    const { startParallelPluginWorker } = await import('./parallel-plugin-worker-runtime');
    const runtimeAndBindingImportFinishedAt = timestamp();
    const afterRuntimeAndBindingImport = captureLauncherProcessMetrics(metricsRuntime);
    const launcherMetrics: WorkerLauncherMetrics = {
      kind: 'rolldown_parallel_plugin_worker_launcher_metrics',
      version: 1,
      metricsId,
      scope:
        'research-only metrics entry before the dynamic import of the worker runtime graph; that graph statically imports binding.cjs',
      timeline: {
        launcherEntryAt,
        metricsRuntimeImportStartedAt,
        metricsRuntimeImportFinishedAt,
        runtimeAndBindingImportStartedAt,
        runtimeAndBindingImportFinishedAt,
      },
      stages: {
        metricsRuntimeImport: stage(metricsRuntimeImportStartedAt, metricsRuntimeImportFinishedAt),
        runtimeAndBindingImport: stage(
          runtimeAndBindingImportStartedAt,
          runtimeAndBindingImportFinishedAt,
        ),
      },
      resources: {
        afterMetricsRuntimeImportBeforeRuntimeAndBindingImport,
        afterRuntimeAndBindingImport,
      },
    };
    await startParallelPluginWorker({ launcherMetrics, metricsRuntime });
  } catch (error) {
    parentPort!.postMessage({ type: 'error', error });
    parentPort!.unref();
  }
})();

function timestamp() {
  const monotonicMs = performance.now();
  return { monotonicMs, epochMs: performance.timeOrigin + monotonicMs };
}

function stage(startedAt: ReturnType<typeof timestamp>, finishedAt: ReturnType<typeof timestamp>) {
  return {
    startedAt,
    finishedAt,
    durationMs: finishedAt.monotonicMs - startedAt.monotonicMs,
  };
}

async function createLauncherMetricsRuntime(): Promise<MetricsRuntime> {
  const [{ performance, PerformanceObserver }, { getHeapStatistics }] = await Promise.all([
    import('node:perf_hooks'),
    import('node:v8'),
  ]);
  return {
    performance,
    getHeapStatistics,
    gcMetrics: createLauncherGcMetricsCollector(PerformanceObserver),
  };
}

function createLauncherGcMetricsCollector(PerformanceObserver: typeof NodePerformanceObserver) {
  const totals = new Map<number, { count: number; durationMs: number; maxDurationMs: number }>();
  let count = 0;
  let durationMs = 0;
  let maxDurationMs = 0;
  const collect = (
    entries: Array<{ duration: number; detail?: { kind?: number }; kind?: number }>,
  ) => {
    for (const entry of entries) {
      const kind = entry.detail?.kind ?? entry.kind ?? 0;
      const value = totals.get(kind) ?? { count: 0, durationMs: 0, maxDurationMs: 0 };
      value.count += 1;
      value.durationMs += entry.duration;
      value.maxDurationMs = Math.max(value.maxDurationMs, entry.duration);
      totals.set(kind, value);
      count += 1;
      durationMs += entry.duration;
      maxDurationMs = Math.max(maxDurationMs, entry.duration);
    }
  };
  const observer = new PerformanceObserver((list) => collect(list.getEntries()));
  observer.observe({ entryTypes: ['gc'] });
  return {
    snapshot: (): GcMetricsSnapshot => {
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

function captureLauncherProcessMetrics(metricsRuntime: MetricsRuntime) {
  return {
    capturedAt: timestamp(),
    scope: {
      cpuUsage: 'whole process, including JS workers and native threads',
      mainThreadCpuUsage: 'current Node.js worker thread only',
      memoryUsage: 'whole process; RSS is not assigned to an isolate or worker',
      heapStatistics: 'current worker V8 isolate only',
      eventLoopUtilization: 'current worker event loop only; this is not CPU time',
      gc: 'GC entries observed in this worker after the research metrics observer started',
    },
    processCpuUsageMicros: process.cpuUsage(),
    mainThreadCpuUsageMicros: process.threadCpuUsage(),
    processResourceUsage: process.resourceUsage(),
    processMemoryUsageBytes: process.memoryUsage(),
    isolateHeapStatistics: metricsRuntime.getHeapStatistics(),
    isolateEventLoopUtilization: metricsRuntime.performance.eventLoopUtilization(),
    isolateGc: metricsRuntime.gcMetrics.snapshot(),
  };
}
