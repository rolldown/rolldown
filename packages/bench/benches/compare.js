import nodeUtil from 'node:util';
import * as bencher from '../src/bencher.js';
import {
  getRolldownSuiteList,
  runEsbuild,
  runRolldown,
  runRollup,
} from '../src/run-bundler.js';
import { expandSuitesWithDerived, suites } from '../src/suites/index.js';

console.log(
  nodeUtil.inspect(suites, { depth: null, colors: true, showHidden: false }),
);

for (const suite of expandSuitesWithDerived(suites)) {
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
