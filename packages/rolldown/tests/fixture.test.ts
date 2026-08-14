import path from 'node:path';
import { test } from 'vitest';
import { compileFixture } from './src/fixture-utils';
import type { TestConfig } from './src/types';

main();

function main() {
  const testConfigPaths = import.meta.glob<TestConfig>('./fixtures/**/_config.ts', {
    import: 'default',
    eager: true,
  });
  for (const [testConfigPath, testConfig] of Object.entries(testConfigPaths)) {
    if (!testConfig.sequential) continue;

    const dirPath = path.dirname(testConfigPath);
    const testName = dirPath.replace('./fixtures/', '');

    test(
      testName,
      {
        skip: testConfig.skip,
        retry: testConfig.retry,
        timeout: 60_000,
      },
      async () => {
        try {
          if (testConfig.beforeTest) {
            await testConfig.beforeTest();
          }
          const output = await compileFixture(
            path.join(import.meta.dirname, dirPath),
            testConfig,
          ).catch(async (err) => {
            if (testConfig.catchError) {
              await testConfig.catchError(err);
              return;
            }
            throw err;
          });
          if (testConfig.afterTest && output) {
            await testConfig.afterTest(output);
          }
        } catch (err) {
          throw new Error(`Failed in ${testConfigPath}`, { cause: err });
        }
      },
    );
  }
}
