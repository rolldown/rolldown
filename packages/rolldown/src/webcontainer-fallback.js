const fs = require('node:fs')
const childProcess = require('node:child_process')

// TODO: support pkg.pr.new?
const rolldownPkg = JSON.parse(
  fs.readFileSync(require.resolve('rolldown/package.json'), 'utf-8'),
)
const version = rolldownPkg.version

const baseDir = `/tmp/rolldown-${version}`
const bindingPkg = `@rolldown/binding-wasm32-wasi@${version}`
const bindingEntry = `${baseDir}/node_modules/@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs`

if (!fs.existsSync(bindingEntry)) {
  fs.rmSync(baseDir, { recursive: true, force: true })
  fs.mkdirSync(baseDir, { recursive: true })
  console.log('Downloading @rolldown/binding-wasm32-wasi on WebContainer...')
  childProcess.execFileSync('pnpm', ['i', bindingPkg], {
    cwd: baseDir,
    stdio: 'inherit',
  })
}

module.exports = require(bindingEntry)
