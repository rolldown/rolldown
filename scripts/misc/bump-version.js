// Derived from https://github.com/web-infra-dev/rspack/blob/ca8afe6ed9ed3c501faf9f38cd5630686dd776d7/scripts/release/version.mjs

import 'zx/globals'
import semver from 'semver'
import path from 'node:path'
import { findWorkspacePackagesNoCheck } from '@pnpm/find-workspace-packages'
import { REPO_ROOT } from '../meta/constants.js'
import fsExtra from 'fs-extra'

/**
 * @typedef {'major' | 'minor' | 'patch'  | 'commit'} PresetVersion
 */

async function getCommitId() {
  const result = await $`git rev-parse --short HEAD`
  return result.stdout.replace('\n', '')
}

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
 * @param {PresetVersion} preset
 */
async function genVersionByPreset(preset) {
  const currentVersion = await getCurrentVersion()
  switch (preset) {
    case 'major':
    case 'minor':
    case 'patch': {
      const v = semver.inc(currentVersion, preset)
      if (!v) {
        throw new Error(`Failed to bump version with preset ${preset}`)
      }
      return v
    }
    case 'commit':
      const commitId = await getCommitId()
      return `${currentVersion}-commit.${commitId}` // Example: 0.15.1-commit.1234567
  }
}

/**
 *
 * @param {string} nextVersion
 */
async function bumpVersion(nextVersion) {
  const root = process.cwd()

  const workspaces = await findWorkspacePackagesNoCheck(root)
  for (const workspace of workspaces) {
    // skip all example upgrade
    if (workspace.manifest.private) {
      continue
    }
    console.info(
      `Bumping ${workspace.manifest.name} from ${workspace.manifest.version} to ${nextVersion}`,
    )
    const newManifest = {
      ...workspace.manifest,
      version: nextVersion ?? undefined,
    }
    workspace.writeProjectManifest(newManifest)
  }
}

/**
 *
 * @param {string} arg
 * @returns {arg is PresetVersion}
 */
function isPresetArg(arg) {
  return ['major', 'minor', 'patch', 'commit'].includes(arg)
}

// --- main

const inputVersion = process.argv[2]?.trim()

if (!inputVersion) {
  throw new Error('You must pass a version to bump')
}

const newVersion = await (async function () {
  if (isPresetArg(inputVersion)) {
    return await genVersionByPreset(inputVersion)
  } else {
    if (!semver.valid(inputVersion)) {
      throw new Error(
        `You must pass a valid semver version instead of '${inputVersion}'`,
      )
    }
    return inputVersion
  }
})()

await bumpVersion(newVersion)
if (process.env.CI) {
  // Write the version to a file for later we can use it in the release process.
  fsExtra.writeFileSync(
    path.resolve(REPO_ROOT, 'rolldown-version.txt'),
    newVersion,
  )
}
