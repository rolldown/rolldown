import * as bencher from '../src/bencher.js';
import {
  getRolldownSuiteList,
  runEsbuild,
  runRolldown,
  runRollup,
} from '../src/run-bundler.js';
import { expandSuitesWithDerived } from '../src/suites/index.js';
import { suiteRomeTsWithBabelAndParallelism } from '../src/suites/rome-ts.js';

for (
  const suite of expandSuitesWithDerived([
    suiteRomeTsWithBabelAndParallelism,
  ])
) {
  const excludedBundlers = Array.isArray(suite.disableBundler)
    ? suite.disableBundler
    : suite.disableBundler
    ? [suite.disableBundler]
    : [];

  const group = bencher.group(suite.title, (bench) => {
    if (!excludedBundlers.includes(`rolldown`)) {
      for (const rolldownSuite of getRolldownSuiteList(suite)) {
        bench.add(`rolldown (${rolldownSuite.suiteName})`, async () => {
          await runRolldown(rolldownSuite);
        });
      }
    }
    if (!excludedBundlers.includes(`esbuild`)) {
      bench.add(`esbuild`, async () => {
        await runEsbuild(suite);
      });
    }
    if (!excludedBundlers.includes(`rollup`)) {
      bench.add(`rollup`, async () => {
        await runRollup(suite);
      });
    }
  });
  const result = await group.run();
  result.display();
}
