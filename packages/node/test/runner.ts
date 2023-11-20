import { describe, expect, test } from 'vitest'
import {
  InputOptions,
  OutputOptions,
  RollupOptions,
  rolldown,
} from '@rolldown/node'
import path from 'path'
import fs from 'fs'

runCases()

function runCases() {
  const testCasesRoot = path.join(__dirname, 'cases')
  const cases = fs.readdirSync(testCasesRoot)

  for (const name of cases) {
    describe(name, async () => {
      const subCasesRoot = path.join(testCasesRoot, name)
      const subCases = fs.readdirSync(subCasesRoot)

      for (const subCaseName of subCases) {
        const caseRoot = path.join(subCasesRoot, subCaseName)
        const { config, afterTest } = await getCaseConfig(caseRoot)

        test(subCaseName, async () => {
          const output = await runCaseBundle(caseRoot, config)
          if (afterTest) {
            afterTest(output)
          }
        })
      }
    })
  }
}

async function runCaseBundle(caseRoot: string, config?: RollupOptions) {
  config = normalizedOptions(caseRoot, config)
  const build = await rolldown(config as InputOptions)
  return await build.write(config.output as OutputOptions)
}

function normalizedOptions(caseRoot: string, config?: RollupOptions) {
  if (Array.isArray(config?.output)) {
    throw new Error(`The ${caseRoot} output shouldn't be array`)
  }
  const output = config?.output ?? {}

  return {
    input: config?.input ?? path.join(caseRoot, 'main.js'),
    output: {
      dir: output.dir ?? path.join(caseRoot, 'dist'),
    },
    ...config,
  }
}

async function getCaseConfig(caseRoot: string) {
  const caseConfigPath = path.join(caseRoot, 'config.ts')
  return fs.existsSync(caseConfigPath)
    ? (await import(caseConfigPath)).default
    : undefined
}
