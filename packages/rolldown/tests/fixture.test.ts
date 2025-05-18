import { test } from 'vitest'
import type { TestConfig } from './src/types'
import { InputOptions, OutputOptions, rolldown } from 'rolldown'
import nodePath from 'node:path'

main()

function main() {
  const fixtureTestConfigPaths = import.meta.glob<TestConfig>(
    './fixtures/**/_config.ts',
    { import: 'default', eager: true },
  );
  
  const pluginTestConfigPaths = import.meta.glob<TestConfig>(
    './fixtures/plugin/**/_config.ts',
    { import: 'default', eager: true },
  );
  
  const onlyFixtureTest = Object.entries(fixtureTestConfigPaths).filter(
    ([_, testConfig]) => testConfig.only,
  );
  
  const onlyPluginTest = Object.entries(pluginTestConfigPaths).filter(
    ([_, testConfig]) => testConfig.only,
  );
  
  let fixtureTests = Object.entries(fixtureTestConfigPaths);
  let pluginTests = Object.entries(pluginTestConfigPaths);
  if (onlyFixtureTest.length + onlyPluginTest.length > 0) {
    fixtureTests = onlyFixtureTest;
    pluginTests = onlyPluginTest;
  }

  for (const [testConfigPath, testConfig] of fixtureTests) {
    const dirPath = nodePath.dirname(testConfigPath)
    const testName = dirPath.replace('./fixtures/', '')

    test.skipIf(testConfig.skip)(testName, async () => {
      try {
        if (testConfig.beforeTest) {
          await testConfig.beforeTest('default')
        }
        const output = await compileFixture(
          nodePath.join(import.meta.dirname, dirPath),
          testConfig,
        ).catch(async (err) => {
          if (testConfig.catchError) {
            await testConfig.catchError(err)
            return
          }
          throw err
        })
        if (testConfig.afterTest && output) {
          await testConfig.afterTest(output)
        }
      } catch (err) {
        throw new Error(`Failed in ${testConfigPath}`, { cause: err })
      }
    })
  }


  for (const [testConfigPath, testConfig] of pluginTests) {
    const dirPath = nodePath.dirname(testConfigPath)
    const testName = dirPath.replace('./fixtures/', '')

    test.skipIf(testConfig.skip || testConfig.skipComposingJsPlugin)(
      `${testName}-composing-js-plugin`,
      async () => {
        testConfig.config = testConfig.config ?? {}
        testConfig.config.experimental = testConfig.config.experimental ?? {}
        testConfig.config.experimental.enableComposingJsPlugins =
          testConfig.config.experimental.enableComposingJsPlugins ?? true
        try {
          if (testConfig.beforeTest) {
            await testConfig.beforeTest('compose-js-plugin')
          }
          const output = await compileFixture(
            nodePath.join(import.meta.dirname, dirPath),
            testConfig,
          ).catch(async (err) => {
            if (testConfig.catchError) {
              await testConfig.catchError(err)
              return
            }
            throw err
          })
          if (testConfig.afterTest && output) {
            await testConfig.afterTest(output)
          }
        } catch (err) {
          throw new Error(`Failed in ${testConfigPath}`, { cause: err })
        }
      },
    )
  }
}

async function compileFixture(fixturePath: string, config: TestConfig) {
  if (Array.isArray(config.config?.output)) {
    throw new Error(
      'The multiply output configure is not support at test runner',
    )
  }
  let outputOptions = config.config?.output ?? {}
  const inputOptions: InputOptions = {
    input: 'main.js',
    cwd: fixturePath,
    ...config.config,
  }
  const build = await rolldown(inputOptions)
  return await build.write(outputOptions as OutputOptions)
}
