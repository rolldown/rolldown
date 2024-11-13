import * as path from 'node:path'
import fsExtra from 'fs-extra'
import { REPO_ROOT } from '../meta/constants.js'

const inputs = `Copy the \`test\`'s value from https://github.com/evanw/bundler-esm-cjs-tests/blob/main/tests.js to here`

const testsFolder = path.join(
  REPO_ROOT,
  'crates/rolldown/tests/rolldown/topics/bundler_esm_cjs_tests',
)

for (const [i, input] of inputs.entries()) {
  const caseFolder = path.join(testsFolder, i.toString())
  for (const [file, content] of Object.entries(input)) {
    fsExtra.outputFileSync(
      path.join(caseFolder, '_config.json'),
      JSON.stringify(
        {
          config: {
            input: [
              {
                name: 'entry',
                import: './entry.js',
              },
            ],
          },
        },
        null,
        2,
      ),
    )
    fsExtra.outputFileSync(
      path.join(caseFolder, '_test.mjs'),
      `globalThis.input = {}
await import('./dist/entry.js')
if (!await input.works) throw new Error('Test did not pass')`,
    )
    fsExtra.outputFileSync(path.join(caseFolder, file), content)
  }
}
