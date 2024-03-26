import { describe, test } from 'vitest'
import { yellow } from 'colorette'
import type { TestConfig } from './types'
import { InputOptions, OutputOptions, RollupOptions, rolldown } from '../src'
import nodePath from 'node:path'
import nodeFs from 'node:fs'
import * as glob from 'glob'

main()

function main() {
  const fixturesPath = nodePath.join(__dirname, 'fixtures')
  const testConfigPaths = glob.globSync(
    nodePath.join(fixturesPath, '**', '_config.ts'),
    { absolute: true },
  )
  for (const testConfigPath of testConfigPaths) {
    const dirName = nodePath.relative(
      fixturesPath,
      nodePath.dirname(testConfigPath),
    )
    test(dirName, async () => {
      const testConfig: TestConfig = await import(testConfigPath).then(
        (m) => m.default,
      )
      const output = await compileFixture(
        nodePath.dirname(testConfigPath),
        testConfig,
      )
      if (testConfig.afterTest) {
        testConfig.afterTest(output)
      }
    })
  }
}

async function compileFixture(fixturePath: string, config: TestConfig) {
  let outputOptions: OutputOptions = config.config?.output ?? {}
  delete config.config?.output
  outputOptions = {
    dir: outputOptions.dir ?? nodePath.join(fixturePath, 'dist'),
    ...outputOptions,
  }

  const inputOptions: InputOptions = {
    input: config.config?.input ?? nodePath.join(fixturePath, 'main.js'),
    ...config.config,
  }
  const build = await rolldown(inputOptions)
  return await build.write(outputOptions)
}
