import { test } from 'vitest'
import type { TestConfig } from './src/types'
import { InputOptions, OutputOptions, rolldown } from 'rolldown'
import nodePath from 'node:path'

main()

function main() {
  const testConfigPaths = import.meta.glob<TestConfig>(
    './fixtures/**/_config.ts',
    { import: 'default', eager: true },
  )
  for (const [testConfigPath, testConfig] of Object.entries(testConfigPaths)) {
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

  for (const [testConfigPath, testConfig] of Object.entries(testConfigPaths)) {
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
  let outputOptions: OutputOptions = config.config?.output ?? {}
  const inputOptions: InputOptions = {
    input: 'main.js',
    cwd: fixturePath,
    ...config.config,
  }
  const build = await rolldown(inputOptions)
  return await build.write(outputOptions)
}
