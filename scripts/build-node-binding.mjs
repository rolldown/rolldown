// @ts-check

import { execa } from 'execa'
import watcher from '@parcel/watcher'
import path from 'path'
import crypto from 'crypto'
import fs from 'fs'
import fsp from 'fs/promises'
import debug from 'debug'

if (!process.env.DEBUG) {
  debug.enable('rolldown')
}

const ROOT_DIR = path.join(path.basename(import.meta.url), '..')
const CACHE_DIR = path.join(ROOT_DIR, 'node_modules/.rolldown')

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

  return result.stdout.split('\n').filter(Boolean)
}

async function generateBuildHash(files) {
  const hasher = crypto.createHash('sha256')

  for (const hash of await hashFiles(files)) {
    hasher.update(hash)
  }

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
  let result = execa('yarn', ['workspace', pkgName, 'run', 'build'], {
    cwd: ROOT_DIR,
    env: { FORCE_COLOR: 'true' },
    stdio: 'pipe',
  })

  result.stdout?.on('data', (data) => {
    String(data)
      .trim()
      .split('\n')
      .forEach((line) => {
        log(`  ${line}`)
      })
  })

  await result
}

async function buildRolldownPackage() {
  const log = debug('rolldown:node')

  log('Checking for changes to @rolldown/node')

  const files = await getDirFiles('packages/node/src')
  files.push(
    'packages/node/build.config.ts',
    'packages/node/package.json',
    'packages/node/tsconfig.json',
  )

  if (await isStaleOrUnbuilt('node', files)) {
    log('Detected changes, building...')

    await runYarnBuild('@rolldown/node', log)
  } else {
    log('No changes since last build')
  }
}

buildRolldownPackage()
