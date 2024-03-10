// @ts-nocheck
import { execa } from 'execa'
import watcher from '@parcel/watcher'
import path from 'node:path'
import crypto from 'crypto'
import fs from 'node:fs'
import fsp from 'fs/promises'
import debug from 'debug'
import chalk from 'chalk'
import { fileURLToPath } from 'node:url'

if (!process.env.DEBUG) {
  debug.enable('rolldown')
}

const ROOT_DIR = path.normalize(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '..'),
)
const CACHE_DIR = path.join(ROOT_DIR, 'node_modules/.rolldown')
const NO_WASM = process.argv.includes('--no-wasm')
const IS_RELEASE = process.argv.includes('--release')

fs.mkdirSync(CACHE_DIR, { recursive: true })

async function getDirFiles(dir) {
  const result = await execa(
    'git',
    [
      'ls-files',
      '--full-name',
      '--cached',
      '--modified',
      '--others',
      '--exclude-standard',
      '--deduplicate',
      dir,
    ],
    { cwd: ROOT_DIR },
  )

  return result.stdout.split('\n').filter(Boolean)
}

async function hashFiles(files) {
  const result = await execa('git', ['hash-object', '--stdin-paths'], {
    cwd: ROOT_DIR,
    input: files.join('\n'),
  })

  return result.stdout
}

function getCacheFile(pkgName) {
  let key = pkgName

  if (key.includes('/')) {
    key = pkgName.split('/')[1]
  }

  return path.join(CACHE_DIR, `hash-${key}`)
}

async function generateBuildHash(deps, files) {
  const hasher = crypto.createHash('sha256')

  for (const dep of deps) {
    const depFile = getCacheFile(dep)

    if (fs.existsSync(depFile)) {
      hasher.update(await fsp.readFile(depFile, 'utf8'))
    }
  }

  hasher.update(await hashFiles(files))

  return hasher.digest('hex')
}

async function isStaleOrUnbuilt(pkgName, deps, files) {
  const cacheFile = getCacheFile(pkgName)
  const buildHash = await generateBuildHash(deps, files)

  if (!fs.existsSync(cacheFile)) {
    await fsp.writeFile(cacheFile, buildHash)
    return true
  }

  const cachedHash = await fsp.readFile(cacheFile, 'utf8')

  if (buildHash != cachedHash) {
    await fsp.writeFile(cacheFile, buildHash)
    return true
  }

  return false
}

async function runYarnBuild(pkgName, log) {
  const command = IS_RELEASE ? 'build:release' : 'build'
  const result = execa('yarn', ['workspace', pkgName, 'run', command], {
    cwd: ROOT_DIR,
    env: { CARGO_TERM_COLOR: 'always', FORCE_COLOR: 'true' },
    stdio: 'pipe',
  })

  const onData = (data) => {
    String(data)
      .split('\n')
      .filter(Boolean)
      .forEach((line) => {
        log(`  ${line.trim()}`)
      })
  }

  result.stderr?.on('data', onData)
  result.stdout?.on('data', onData)

  try {
    await result
  } catch (err) {
    debug(`Yarn Build:error`)(`Failed to build ${pkgName}: ${err.message}`)
    process.exitCode = 1
  }
}

const BUILD_TIMERS = {}

async function build(pkgName, deps, changedFile, loadFiles) {
  const name = pkgName.split('/')[1]
  const log = debug(`rolldown:${name}`)

  if (!changedFile) {
    log(`Checking for changes to ${pkgName}`)
  }

  const files = await loadFiles()

  // Exit early for files we don't care about
  if (changedFile && !files.some((file) => file === changedFile)) {
    return false
  }

  if (await isStaleOrUnbuilt(name, deps, files)) {
    log('Detected changes, building...')

    await runYarnBuild(pkgName, log)

    return true
  }

  log('No changes since last build')

  return false
}

async function buildWithDebounce(pkgName, deps, changedFile, loadFiles) {
  if (BUILD_TIMERS[pkgName]) {
    const [resolve, timer] = BUILD_TIMERS[pkgName]
    clearTimeout(timer)
    resolve()
    delete BUILD_TIMERS[pkgName]
  }

  return new Promise((resolve) => {
    BUILD_TIMERS[pkgName] = [
      resolve,
      setTimeout(
        () => resolve(build(pkgName, deps, changedFile, loadFiles)),
        125,
      ),
    ]
  })
}

async function buildRolldownPackage(changedFile) {
  return buildWithDebounce(
    '@rolldown/node',
    ['@rolldown/node-binding'],
    changedFile,
    async () => {
      const files = await getDirFiles('packages/node/src')
      files.push(
        'packages/node/build.config.ts',
        'packages/node/package.json',
        'packages/node/tsconfig.json',
      )
      return files
    },
  )
}

async function buildNodeBindingCrate(changedFile) {
  return buildWithDebounce(
    '@rolldown/node-binding',
    [],
    changedFile,
    async () => {
      const files = await getDirFiles('crates/rolldown_binding/src')
      files.push(
        'crates/rolldown_binding/build.rs',
        'crates/rolldown_binding/Cargo.toml',
        'crates/rolldown_binding/package.json',
      )
      return files
    },
  )
}

async function buildWasmBindingCrate(changedFile) {
  if (NO_WASM) {
    return
  }

  return buildWithDebounce(
    '@rolldown/wasm-binding',
    [],
    changedFile,
    async () => {
      const files = await getDirFiles('crates/rolldown_binding_wasm/src')
      files.push(
        'crates/rolldown_binding_wasm/Cargo.toml',
        'crates/rolldown_binding_wasm/package.json',
      )
      return files
    },
  )
}

async function watchForChanges() {
  const log = debug('rolldown:watcher')

  log('Watching for changes...')

  const onChange = (error, events) => {
    if (error) {
      console.error(error)
      process.exit(1)
    }

    events.forEach((event) => {
      const changedFile = event.path.replace(ROOT_DIR, '').slice(1)

      log(chalk.gray(`${event.type}: ${changedFile}`))

      if (changedFile.includes('crates/rolldown_binding/')) {
        buildNodeBindingCrate(changedFile).then(() =>
          buildRolldownPackage(changedFile),
        )
      } else if (changedFile.includes('crates/rolldown_binding_wasm/')) {
        buildWasmBindingCrate(changedFile)
      } else if (changedFile.includes('packages/node/')) {
        buildRolldownPackage(changedFile)
      }
    })
  }

  const promises = [
    watcher.subscribe(path.join(ROOT_DIR, 'packages/node'), onChange),
    watcher.subscribe(path.join(ROOT_DIR, 'crates/rolldown_binding'), onChange),
  ]

  if (!NO_WASM) {
    promises.push(
      watcher.subscribe(
        path.join(ROOT_DIR, 'crates/rolldown_binding_wasm'),
        onChange,
      ),
    )
  }

  await Promise.all(promises)
}

await Promise.all([
  buildNodeBindingCrate().then(() => buildRolldownPackage()),
  buildWasmBindingCrate(),
])

if (process.argv.includes('--watch')) {
  await watchForChanges()
}
