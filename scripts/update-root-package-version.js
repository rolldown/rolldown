// The `lerna version` command don't update the root package.json version,
// We use it to generate changelog, so here update it by packages version.
const fs = require('fs')
const path = require('path')

const updatedVersion = require('../packages/node/package.json').version
const pkgPath = path.join(__dirname, '../package.json')
const packageJson = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'))
packageJson.version = updatedVersion
fs.writeFileSync(pkgPath, JSON.stringify(packageJson, null, 2))
