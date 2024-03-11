const fs = require('node:fs/promises')
const path = require('node:path')
const { execSync } = require('node:child_process')

// The `lerna version` command don't update the root package.json version,
// We use it to generate changelog, so here update it by packages version.
const ROOT_DIR = path.join(__dirname, '..')
const PACKAGE_JSON_PATH = path.join(ROOT_DIR, 'package.json')
const NODE_PACKAGE_JSON_PATH = path.join(
  ROOT_DIR,
  'packages/rolldown/package.json',
)

async function updatePackageVersion() {
  const { version: updatedVersion } = require(NODE_PACKAGE_JSON_PATH)
  const packageJson = JSON.parse(await fs.readFile(PACKAGE_JSON_PATH, 'utf-8'))
  packageJson.version = updatedVersion
  await fs.writeFile(PACKAGE_JSON_PATH, JSON.stringify(packageJson, null, 2))
  return updatedVersion
}

// Generate changelog
function generateChangelog() {
  execSync('npm run changelog', { stdio: 'inherit' })
}

// Add git tag, the tag name is `v${version}`, the changelog will generate by tag.
/**
 *
 * @param {string} version
 */
function addGitTag(version) {
  execSync(`git tag v${version}`, { stdio: 'inherit' })
}

// Add commit, the napi will check comment to publish, avoid `No release commit found` error.
// TODO maybe napi-rs can fix it.
/**
 *
 * @param {string} version
 */
function addReleaseCommit(version) {
  execSync(
    `git commit -a -m "chore(release): publish \n- @rolldown/node-binding@${version}"`,
    { stdio: 'inherit' },
  )
}

async function main() {
  try {
    const { version: updatedVersion } = await updatePackageVersion()
    generateChangelog()
    addGitTag(updatedVersion)
    addReleaseCommit(updatedVersion)

    console.log('Release process completed successfully!')
  } catch (err) {
    console.error('Error occurred during release process:', err)
    process.exitCode = 1
  }
}

main()
