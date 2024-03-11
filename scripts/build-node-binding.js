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

/** @typedef {import('debug').Debugger['log']} LogFunction  */
/** A function that returns an array of paths for files (similar to the getDirFiles function, see {@link getDirFiles}) @typedef {() => Promise<string[]>} LoadFilesCallback */
/** The file that triggered the build. @typedef {string} ChangedFile */
/** Represents whether the package was rebuilt. @typedef {boolean} WasRebuilt */

/**
 * Takes a path relative to the project root and returns an array of paths for files in this directory.
 * @param {string} dir - Directory path relative to the project root directory. For example, 'crates/rolldown_binding/src'.
 * @returns {Promise<string[]>} Array of directory file paths relative to the rolldown root directory.
 * Example: If dir is 'crates/rolldown_binding/src', it returns ['crates/rolldown_binding/src/bunder.rs', 'crates/rolldown_binding/src/lib.rs', and so on].
 *
 * @example
 * const files = await getDirFiles('crates/rolldown_binding/src')
 * console.log(files) // Example output: ['crates/rolldown_binding/src/bunder.rs', 'crates/rolldown_binding/src/lib.rs', and so on]
 */
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

/**
 * Takes an array of file paths and returns a common hash string.
 * @param {string[]} files - Array of file paths.
 * @returns {Promise<string>} A common hash string for input files.
 */
async function hashFiles(files) {
  const result = await execa('git', ['hash-object', '--stdin-paths'], {
    cwd: ROOT_DIR,
    input: files.join('\n'),
  })

  return result.stdout
}

/**
 * Returns the path to the cache file for the given package name.
 * @param {string} pkgName - The package name.
 * @returns {string} The absolute path to the cache file.
 */
function getCacheFile(pkgName) {
  let key = pkgName

  if (key.includes('/')) {
    key = pkgName.split('/')[1]
  }

  return path.join(CACHE_DIR, `hash-${key}`)
}

/**
 * Generates the build hash string
 * @param {string[]} deps - An array of dependencies.
 * @param {string[]} files - An array of file paths.
 * @returns {Promise<string>} The build hash string
 */
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

/**
 * Checks if the package needs to be rebuilt. If the cache is stale or does not exist, it returns true.
 * @param {string} pkgName - The package name
 * @param {string[]} deps - An array of dependencies.
 * @param {string[]} files - An array of file paths.
 * @returns {Promise<boolean>} Indicates whether the package needs to be rebuilt.
 */
async function isStaleOrUnbuilt(pkgName, deps, files) {
  const cacheFile = getCacheFile(pkgName)
  const buildHash = await generateBuildHash(deps, files)

  // If the build does not exist
  if (!fs.existsSync(cacheFile)) {
    await fsp.writeFile(cacheFile, buildHash)
    return true
  }

  const cachedHash = await fsp.readFile(cacheFile, 'utf8')

  // If the cached and build hashes don't match, the build is stale
  if (buildHash != cachedHash) {
    await fsp.writeFile(cacheFile, buildHash)
    return true
  }

  // Otherwise, the package is up-to-date
  return false
}

/**
 * Executes the 'yarn build' command for the specified package.
 * @param {string} pkgName - The package name
 * @param {LogFunction} log - Log function
 * @returns {Promise<void>}
 */
async function runYarnBuild(pkgName, log) {
  const command = IS_RELEASE ? 'build:release' : 'build'
  const result = execa('yarn', ['workspace', pkgName, 'run', command], {
    cwd: ROOT_DIR,
    env: { CARGO_TERM_COLOR: 'always', FORCE_COLOR: 'true' },
    stdio: 'pipe',
  })

  /**
   * Callback function called when a chunk of data is received from the stream.
   * @param {unknown} data - The chunk of data received from the stream.
   */
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
    if (err instanceof Error) {
      debug(`Yarn Build:error`)(`Failed to build ${pkgName}: ${err.message}`)
    } else {
      debug(`Yarn Build:error`)(`Failed to build ${pkgName}: ${err}`)
    }
    process.exitCode = 1
  }
}

/**
 * Object with keys representing package names and values as tuples consisting of the resolve function for {@link build} function and the setTimeout timer.
 * Used in the 'buildWithDebounce' function to manage debounced builds for different packages.
 * @type {{ [packageName: string]: [resolve: (value: boolean | PromiseLike<boolean>) => void, timeout: ReturnType<setTimeout>] }}
 */
const BUILD_TIMERS = {}

/**
 * Builds the specified package.
 * @param {string} pkgName - The name of the package to build.
 * @param {string[]} deps - An array of dependencies required for building the package.
 * @param {ChangedFile | undefined} changedFile - Optional. The file that triggered the build, if any.
 * @param {LoadFilesCallback} loadFiles - A function that returns an array of paths for files (similar to the getDirFiles function, see {@link getDirFiles})
 * @returns {Promise<WasRebuilt>} A Promise that resolves to true if the package was rebuilt, false otherwise.
 */
async function build(pkgName, deps, changedFile, loadFiles) {
  const name = pkgName?.split('/')[1] || pkgName
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

/**
 *
 * @param {string} pkgName - The package name
 * @param {string[]} deps - An array of dependencies.
 * @param {ChangedFile | undefined} changedFile - Optional. The file that triggered the build, if any.
 * @param {LoadFilesCallback} loadFiles - A function that returns an array of paths for files (similar to the getDirFiles function, see {@link getDirFiles})
 * @returns {Promise<WasRebuilt>} A Promise that resolves to true if the package was rebuilt, false otherwise.
 */
async function buildWithDebounce(pkgName, deps, changedFile, loadFiles) {
  if (BUILD_TIMERS[pkgName]) {
    const [resolve, timer] = BUILD_TIMERS[pkgName]
    clearTimeout(timer)
    resolve(true)
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

/**
 *
 * @param {ChangedFile} [changedFile] - Optional. The file that triggered the build, if any.
 * @returns {Promise<WasRebuilt>} A Promise that resolves to true if the package was rebuilt, false otherwise.
 */
async function buildRolldownPackage(changedFile) {
  return buildWithDebounce(
    'rolldown',
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

/**
 * @param {ChangedFile} [changedFile] - Optional. The file that triggered the build, if any.
 * @returns {Promise<WasRebuilt>} A Promise that resolves to true if the package was rebuilt, false otherwise.
 */
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

/**
 * @param {ChangedFile} [changedFile] - Optional. The file that triggered the build, if any.
 * @returns {Promise<WasRebuilt>} A Promise that resolves to true if the package was rebuilt, false otherwise.
 */
async function buildWasmBindingCrate(changedFile) {
  if (NO_WASM) {
    return false
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

/**
 * Subscribe to changes in subfolders and rebuild some package if change is detected.
 * @returns {Promise<void>}
 */
async function watchForChanges() {
  const log = debug('rolldown:watcher')

  log('Watching for changes...')

  /** @type import('@parcel/watcher').SubscribeCallback */
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
    return
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
