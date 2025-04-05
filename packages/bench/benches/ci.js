import nodeFs from 'node:fs';
import nodePath from 'node:path';
import nodeUrl from 'node:url';
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

for (const suite of expandSuitesWithDerived(suitesForCI)) {
  const rolldownSuiteList = getRolldownSuiteList(suite);
  for (const rolldownSuite of rolldownSuiteList) {
    bench.add(`${suite.title} (${rolldownSuite.suiteName})`, async () => {
      await runRolldown(rolldownSuite);
    });
  }
}

await bench.run();

const dataForGitHubBenchmarkAction = bench.tasks.map((task) => {
  if (!task.result) {
    throw new Error('Task result is empty for ' + task.name);
  }

  return {
    name: task.name,
    value: task.result.mean.toFixed(2),
    unit: 'ms / ops',
  };
});

const serialized = JSON.stringify(dataForGitHubBenchmarkAction, null, 2);

console.log(serialized);

nodeFs.writeFileSync(
  nodePath.resolve(REPO_ROOT, 'tmp/new-benchmark-node-output.json'),
  serialized,
  'utf8',
);

// TODO: avoid hanging benchmark-node in CI
process.exit(0);
