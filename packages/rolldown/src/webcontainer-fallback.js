const fs = require('node:fs')
const childProcess = require('node:child_process')

const rolldownPkg = JSON.parse(
  fs.readFileSync(require.resolve('rolldown/package.json'), 'utf-8'),
)
const version = rolldownPkg.version
const baseDir = `/tmp/rolldown-${version}`
const bindingEntry = `${baseDir}/node_modules/@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs`

if (!fs.existsSync(bindingEntry)) {
  let bindingPkg = `@rolldown/binding-wasm32-wasi@${version}`

  try {
    // check if pkg.pr.new
    const info = JSON.parse(
      childProcess.execFileSync('npm', ['why', '--json', 'rolldown'], {
        encoding: 'utf-8',
      }),
    )
    const spec = info[0].dependents[0].spec
    if (spec.startsWith('https://pkg.pr.new/')) {
      const commit = spec.split('@').at(-1)
      bindingPkg = `https://pkg.pr.new/@rolldown/binding-wasm32-wasi@${commit}`
    }
  } catch (e) {
    console.error(e)
  }

  fs.rmSync(baseDir, { recursive: true, force: true })
  fs.mkdirSync(baseDir, { recursive: true })
  // eslint-disable-next-line: no-console
  console.log(`Downloading ${bindingPkg} on WebContainer...`)
  childProcess.execFileSync('pnpm', ['i', bindingPkg], {
    cwd: baseDir,
    stdio: 'inherit',
  })
}

module.exports = require(bindingEntry)
