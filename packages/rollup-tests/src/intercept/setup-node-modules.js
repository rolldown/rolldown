const fs = require('node:fs')
const path = require('node:path')

// link rollup/node_modules to rollup-tests/node_modules
// We want to control the versions of dependencies to ensure the minimumRelease and other gates are satisfied
const rollupNodeModules = path.resolve(__dirname, '../../../../rollup/node_modules')
const rollupTestsNodeModules = path.resolve(__dirname, '../../node_modules')

let linkResult
try {
  linkResult = fs.readlinkSync(rollupNodeModules)
} catch (err) {
  if (/** @type {any} */ (err).code !== 'ENOENT') throw err
}
if (linkResult && linkResult !== rollupTestsNodeModules) {
  fs.unlinkSync(rollupNodeModules)
}
if (!linkResult || linkResult !== rollupTestsNodeModules) {
  fs.symlinkSync(rollupTestsNodeModules, rollupNodeModules)
}
