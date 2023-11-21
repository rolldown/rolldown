// @ts-check

import { execa } from 'execa'
import watcher from '@parcel/watcher'
import path from 'path'
import crypto from 'crypto'
import fs from 'fs'
import fsp from 'fs/promises'
import debug from 'debug'
import chalk from 'chalk'
import { fileURLToPath } from 'url'

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

async function generateBuildHash(files) {
  const hasher = crypto.createHash('sha256')

  hasher.update(await hashFiles(files))

  return hasher.digest('hex')
}

async function isStaleOrUnbuilt(key, files) {
  const cacheFile = path.join(CACHE_DIR, `hash-${key}`)
  const buildHash = await generateBuildHash(files)

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
  const result = execa(
    'yarn',
    ['workspace', pkgName, 'run', IS_RELEASE ? 'build:release' : 'build'],
    {
      cwd: ROOT_DIR,
      env: { CARGO_TERM_COLOR: 'always', FORCE_COLOR: 'true' },
      stdio: 'pipe',
    },
  )

  const onData = (data) => {
    String(data)
      .trim()
      .split('\n')
      .forEach((line) => {
        if (line) {
          log(`  ${line.trim()}`)
        }
      })
  }

  result.stderr?.on('data', onData)
  result.stdout?.on('data', onData)

  await result
}

const BUILD_TIMERS = {}

async function build(pkgName, changedFile, loadFiles) {
  const name = pkgName.split('/')[1]
  const log = debug(`rolldown:${name}`)

  if (!changedFile) {
    log(`Checking for changes to ${pkgName}`)
  }

  const files = await loadFiles()

  // Exit early for files we dont care about
  if (changedFile && !files.some((file) => file === changedFile)) {
    return false
  }

  if (await isStaleOrUnbuilt(name, files)) {
    log('Detected changes, building...')

    await runYarnBuild(pkgName, log)

    return true
  }

  log('No changes since last build')

  return false
}

async function buildWithDebounce(pkgName, changedFile, loadFiles) {
  if (BUILD_TIMERS[pkgName]) {
    const [resolve, timer] = BUILD_TIMERS[pkgName]
    clearTimeout(timer)
    resolve()
    delete BUILD_TIMERS[pkgName]
  }

  return new Promise((resolve) => {
    BUILD_TIMERS[pkgName] = [
      resolve,
      setTimeout(() => resolve(build(pkgName, changedFile, loadFiles)), 125),
    ]
  })
}

async function buildRolldownPackage(changedFile) {
  buildWithDebounce('@rolldown/node', changedFile, async () => {
    const files = await getDirFiles('packages/node/src')
    files.push(
      'packages/node/build.config.ts',
      'packages/node/package.json',
      'packages/node/tsconfig.json',
    )
    return files
  })
}

async function buildNodeBindingCrate(changedFile) {
  buildWithDebounce('@rolldown/node-binding', changedFile, async () => {
    const files = await getDirFiles('crates/rolldown_binding/src')
    files.push(
      'crates/rolldown_binding/build.rs',
      'crates/rolldown_binding/Cargo.toml',
      'crates/rolldown_binding/package.json',
    )
    return files
  })
}

async function buildWasmBindingCrate(changedFile) {
  if (NO_WASM) {
    return
  }

  buildWithDebounce('@rolldown/wasm-binding', changedFile, async () => {
    const files = await getDirFiles('crates/rolldown_binding_wasm/src')
    files.push(
      'crates/rolldown_binding_wasm/Cargo.toml',
      'crates/rolldown_binding_wasm/package.json',
    )
    return files
  })
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
        buildNodeBindingCrate(changedFile)
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
