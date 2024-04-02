// Derived from https://github.com/web-infra-dev/rspack/blob/ca8afe6ed9ed3c501faf9f38cd5630686dd776d7/scripts/release/version.mjs

import 'zx/globals'
import semver from 'semver'
import path from 'path'
import { findWorkspacePackagesNoCheck } from '@pnpm/find-workspace-packages'
import { REPO_ROOT } from '../meta/constants.js'

async function getCommitId() {
  const result = await $`git rev-parse --short HEAD`
  return result.stdout.replace('\n', '')
}

async function getLastVersion() {
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
 * @param {string} lastVersion
 */
async function getSnapshotVersion(lastVersion) {
  const commitId = await getCommitId()
  const dateTime = new Date()
    .toISOString()
    .replace(/\.\d{3}Z$/, '')
    .replace(/[^\d]/g, '')
  return `${lastVersion}-snapshot-${commitId}-${dateTime}`
}

/**
 *
 * @param {'major' | 'minor' | 'patch' | 'snapshot'} version
 */
async function bumpVersion(version) {
  const allowedVersion = ['major', 'minor', 'patch', 'snapshot']
  if (!allowedVersion.includes(version)) {
    throw new Error(
      `version must be one of ${allowedVersion}, but you passed ${version}`,
    )
  }
  const root = process.cwd()

  const lastVersion = await getLastVersion()
  const nextVersion = await (() => {
    if (version === 'snapshot') {
      return getSnapshotVersion(lastVersion)
    } else {
      return semver.inc(lastVersion, version)
    }
  })()

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

const version = process.argv[2]

if (!version) {
  console.error(
    "You must pass a version to bump. e.g. 'major', 'minor', 'patch', 'canary'",
  )
  process.exit(1)
}

// @ts-expect-error
await bumpVersion(version)
