import { $, ProcessOutput } from 'zx'
import { assertRunningScriptFromRepoRoot } from '../meta/utils.js'
import * as actionsCore from '@actions/core'
import { REPO_ROOT } from '../meta/constants.js'

/**
 *
 * @returns {Promise<string>}
 */
async function getCurrentVersion() {
  const pkgPath = path.resolve(REPO_ROOT, './packages/rolldown/package.json')
  const result = await import(pkgPath, {
    assert: {
      type: 'json',
    },
  })
  return result.default.version
}

/**
 *
 * @param {string} version
 */
async function isVersionExists(version) {
  try {
    const result = await $({ quiet: true })`npm show rolldown@${version}`
    return result.exitCode === 0
  } catch (cause) {
    if (
      cause instanceof ProcessOutput &&
      cause.stderr.includes('No match found for version')
    ) {
      return false
    }
    throw new Error(
      `Unexpected error happened when checking if rolldown@${version} exist.`,
      { cause },
    )
  }
}

/**
 *
 * @param {string} version
 * @param {string} tag
 */
async function publish(version, tag) {
  if (await isVersionExists(version)) {
    await $`pnpm dist-tag add rolldown@${version} ${tag}`
    actionsCore.info(`Version ${version} exists, just add dist-tag ${tag}`)
    return
  }
  // Let's try dry-run first
  await $`pnpm publish -r --tag ${tag} --dry-run --no-git-checks`
  await $`pnpm publish -r --tag ${tag} --no-git-checks`
}

// --- main

assertRunningScriptFromRepoRoot()

const tag = process.argv[2]?.trim()

if (!tag) {
  throw new Error('Npm tag must be provided')
}

publish(await getCurrentVersion(), tag)
