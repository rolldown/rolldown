import nodePath from 'node:path'
import { test } from 'vitest'
import { InputOptions, OutputOptions, rolldown } from 'rolldown'
import type { TestConfig } from './src/types'

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
        const output = await compileFixture(
          nodePath.join(import.meta.dirname, dirPath),
          testConfig,
        )
        if (testConfig.afterTest) {
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

    test.skipIf(testConfig.skipComposingJsPlugin)(
      `${testName}-composing-js-plugin`,
      async () => {
        testConfig.config = testConfig.config ?? {}
        testConfig.config.experimental = testConfig.config.experimental ?? {}
        testConfig.config.experimental.enableComposingJsPlugins =
          testConfig.config.experimental.enableComposingJsPlugins ?? true
        try {
          const output = await compileFixture(
            nodePath.join(import.meta.dirname, dirPath),
            testConfig,
          )
          if (testConfig.afterTest) {
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
