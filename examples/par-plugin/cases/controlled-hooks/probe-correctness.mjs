import { spawnSync } from 'node:child_process';
import nodePath from 'node:path';

if (process.version !== 'v24.18.0') {
  throw new Error(`correctness probes require Node v24.18.0, got ${process.version}`);
}

const childPath = nodePath.join(import.meta.dirname, 'run-probe-child.mjs');
const run = (variant, mode, timeout = 10000, metrics = false) => {
  const environment = { ...process.env };
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS;
  delete environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS;
  const workerMatch = /^worker-(\d+)$/.exec(variant);
  if (workerMatch) {
    environment.ROLLDOWN_PARALLEL_PLUGIN_WORKERS = workerMatch[1];
    if (metrics) environment.ROLLDOWN_PARALLEL_PLUGIN_METRICS = 'json';
  }
  return spawnSync(process.execPath, [childPath, JSON.stringify({ variant, mode })], {
    encoding: 'utf8',
    env: environment,
    timeout,
  });
};

const successful = (variant, mode, timeout, metrics) => {
  const result = run(variant, mode, timeout, metrics);
  if (result.status !== 0) {
    throw new Error(`${variant}/${mode} failed:\n${result.stderr}`);
  }
  return { result, output: JSON.parse(result.stdout) };
};

const filterMiss = successful('worker-1', 'filter-miss', 10000, true);
const rustMetricsMatch = filterMiss.result.stderr.match(
  /^\[rolldown-parallel-plugin-metrics\] (\{.*\})$/m,
);
if (!rustMetricsMatch) throw new Error('filter miss probe did not emit Rust hook metrics');
const filterMetrics = JSON.parse(rustMetricsMatch[1]);
if (
  filterMetrics.hook !== 'resolveId' ||
  filterMetrics.wrapperCalls < 1 ||
  filterMetrics.nullResults !== filterMetrics.wrapperCalls ||
  filterMetrics.permitAcquiredCalls !== filterMetrics.wrapperCalls
) {
  throw new Error('native filter miss did not acquire and return a worker permit as expected');
}

const ordinaryState = successful('ordinary', 'state').output;
const workerState = successful('worker-4', 'state').output;
if (ordinaryState.stateTuples.length !== 32 || workerState.stateTuples.length !== 32) {
  throw new Error('state probe did not preserve all 32 modules');
}
if (ordinaryState.statePerWorkerCalls.filter(Boolean).length !== 1) {
  throw new Error('ordinary state probe unexpectedly used multiple instances');
}
const ordinaryLocalCounters = ordinaryState.stateTuples.map(([call]) => call);
if (new Set(ordinaryLocalCounters).size !== 32) {
  throw new Error('ordinary instance did not produce one unique local counter per call');
}
const workerThreads = workerState.statePerWorkerCalls.flatMap((calls, thread) =>
  calls > 0 ? [thread] : [],
);
if (workerThreads.length < 2) throw new Error('worker state probe did not distribute work');
const workerLocalCounters = workerState.stateTuples.map(([call]) => call);
if (new Set(workerLocalCounters).size === 32) {
  throw new Error('worker-local closure counters unexpectedly behaved like one shared counter');
}
if (ordinaryState.outputHash === workerState.outputHash) {
  throw new Error('per-instance state did not produce the expected observable semantic difference');
}

const ordinaryReentrant = successful('ordinary', 'reentrant').output;
const workerTwoReentrant = successful('worker-2', 'reentrant').output;
if (ordinaryReentrant.outputHash !== workerTwoReentrant.outputHash) {
  throw new Error('successful reentrant variants produced different output');
}
const workerOneReentrant = run('worker-1', 'reentrant', 2000);
if (workerOneReentrant.error?.code !== 'ETIMEDOUT') {
  throw new Error(
    `worker-1 reentrant probe should time out while waiting for its own permit, got status=${workerOneReentrant.status} error=${workerOneReentrant.error?.code}`,
  );
}

for (const mode of ['resolve-error', 'load-error']) {
  const marker = mode === 'resolve-error' ? 'controlled resolveId error' : 'controlled load error';
  for (const variant of ['ordinary', 'worker-1']) {
    const result = run(variant, mode);
    if (result.status === 0 || result.signal || !result.stderr.includes(marker)) {
      throw new Error(`${variant}/${mode} did not propagate the controlled hook error`);
    }
  }
}

console.log(
  JSON.stringify(
    {
      node: process.version,
      filterMiss: {
        wrapperCalls: filterMetrics.wrapperCalls,
        permitAcquiredCalls: filterMetrics.permitAcquiredCalls,
        nullResults: filterMetrics.nullResults,
      },
      state: {
        ordinaryOutputHash: ordinaryState.outputHash,
        workerOutputHash: workerState.outputHash,
        perWorkerCalls: workerState.statePerWorkerCalls,
      },
      reentrant: {
        ordinaryAndWorkerTwoHash: ordinaryReentrant.outputHash,
        workerOne: 'timed out after 2000 ms while holding the only permit',
      },
      errors: ['ordinary resolveId', 'worker-1 resolveId', 'ordinary load', 'worker-1 load'],
    },
    null,
    2,
  ),
);
