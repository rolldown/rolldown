const fs = require('node:fs')
const childProcess = require('node:child_process')

const rolldownPkg = JSON.parse(
  fs.readFileSync(require.resolve('rolldown/package.json'), 'utf-8'),
)
const version = rolldownPkg.version
const baseDir = `/tmp/rolldown-${version}`
const bindingEntry = `${baseDir}/node_modules/@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs`

if (!fs.existsSync(bindingEntry)) {
  const bindingPkg = `@rolldown/binding-wasm32-wasi@${version}`
  fs.rmSync(baseDir, { recursive: true, force: true })
  fs.mkdirSync(baseDir, { recursive: true })
  // eslint-disable-next-line: no-console
  console.log(`[rolldown] Downloading ${bindingPkg} on WebContainer...`)
  childProcess.execFileSync('pnpm', ['i', bindingPkg], {
    cwd: baseDir,
    stdio: 'inherit',
  })
}

module.exports = require(bindingEntry)
