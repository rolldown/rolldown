import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
import { getNativeMemoryStats, resetNativeMemoryStats } from 'rolldown/experimental';
import * as tinyBench from 'tinybench';
import { getRolldownSuiteList, runRolldown } from '../src/run-bundler.js';
import { expandSuitesWithDerived, suitesForCI } from '../src/suites/index.js';

const DIRNAME = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url));
const PROJECT_ROOT = nodePath.resolve(DIRNAME, '..');
const REPO_ROOT = nodePath.resolve(PROJECT_ROOT, '../..');

const bench = new tinyBench.Bench({
  iterations: 10,
  warmupIterations: 5,
});
bench.threshold = 1;

// Peak Rust-side memory per suite, in bytes. Only recorded when the binding was
// built with `--features tracking_allocator` (`just build-rolldown-release-tracking`,
// which the benchmark workflow uses); a stock binding returns null and no memory
// rows are emitted.
const peakMemoryBySuite = new Map();

for (const suite of expandSuitesWithDerived(suitesForCI)) {
  const rolldownSuiteList = getRolldownSuiteList(suite);
  for (const rolldownSuite of rolldownSuiteList) {
    const taskName = `${suite.title} (${rolldownSuite.suiteName})`;
    // Suites share the process, so memory retained by earlier suites is the
    // floor this suite starts from. Record the peak ABOVE that floor — the
    // suite's own demand — so later suites are not charged for earlier residue.
    let liveAtStart = 0;
    bench.add(
      taskName,
      async () => {
        await runRolldown(rolldownSuite);
      },
      {
        beforeAll: () => {
          resetNativeMemoryStats();
          liveAtStart = getNativeMemoryStats()?.liveBytes ?? 0;
        },
        afterAll: () => {
          const stats = getNativeMemoryStats();
          if (stats) {
            peakMemoryBySuite.set(taskName, Math.max(0, Math.round(stats.peakBytes - liveAtStart)));
          }
        },
      },
    );
  }
}

await bench.run();

const dataForGitHubBenchmarkAction = bench.tasks.map((task) => {
  if (!task.result || !('latency' in task.result)) {
    throw new Error('Task result is empty for ' + task.name);
  }

  return {
    name: task.name,
    value: task.result.latency.mean.toFixed(2),
    unit: 'ms / ops',
  };
});

for (const [taskName, peakBytes] of peakMemoryBySuite) {
  dataForGitHubBenchmarkAction.push({
    name: `${taskName} (peak memory)`,
    value: peakBytes,
    unit: 'bytes',
  });
}

const serialized = JSON.stringify(dataForGitHubBenchmarkAction, null, 2);

console.log(serialized);

nodeFs.writeFileSync(
  nodePath.resolve(REPO_ROOT, 'tmp/new-benchmark-node-output.json'),
  serialized,
  'utf8',
);

// TODO: avoid hanging benchmark-node in CI
process.exit(0);
