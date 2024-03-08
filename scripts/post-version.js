const fs = require('node:fs')
const path = require('node:path')
const { execSync } = require('node:child_process')

// The `lerna version` command don't update the root package.json version,
// We use it to generate changelog, so here update it by packages version.
const updatedVersion = require('../packages/node/package.json').version
const pkgPath = path.join(__dirname, '../package.json')
const packageJson = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'))
packageJson.version = updatedVersion
fs.writeFileSync(pkgPath, JSON.stringify(packageJson, null, 2))

// Generate changelog
execSync('npm run changelog', { stdio: 'inherit' })
// Add git tag, the tag name is `v${version}`, the changelog will generate by tag.
execSync(`git tag v${updatedVersion}`, { stdio: 'inherit' })
// Add commit, the napi will check comment to publish, avoid `No release commit found` error.
// TODO maybe napi-rs can fix it.
execSync(
  `git commit -a -m "chore(release): publish 
- @rolldown/node-binding@${updatedVersion}`,
  { stdio: 'inherit' },
)
