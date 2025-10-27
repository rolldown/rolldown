import path from 'node:path'
import { test } from 'vitest'
import { rolldown } from 'rolldown'
import type { InputOptions } from 'rolldown'
import type { TestConfig } from './src/types'

main()

function main() {
  const testConfigPaths = import.meta.glob<TestConfig>(
    './fixtures/**/_config.ts',
    { import: 'default', eager: true },
  )
  for (const [testConfigPath, testConfig] of Object.entries(testConfigPaths)) {
    const dirPath = path.dirname(testConfigPath)
    const testName = dirPath.replace('./fixtures/', '')

    test.skipIf(testConfig.skip)(testName, async () => {
      try {
        if (testConfig.beforeTest) {
          await testConfig.beforeTest()
        }
        const output = await compileFixture(
          path.join(import.meta.dirname, dirPath),
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
}

async function compileFixture(fixturePath: string, config: TestConfig) {
  const inputOptions: InputOptions = {
    input: 'main.js',
    cwd: fixturePath,
    ...config.config,
  }
  const build = await rolldown(inputOptions)
  if (Array.isArray(config.config?.output)) {
    const outputs = []
    for (const output of config.config.output) {
      outputs.push(await build.write(output))
    }
    return outputs
  }
  const outputOptions = config.config?.output ?? {}
  return await build.write(outputOptions)
}
