const esbuild = require('esbuild')
const path = require('path')

async function esbuildBundle(entryPoints) {
  await esbuild.build({
    entryPoints: entryPoints,
    bundle: true,
    outfile: 'out.js',
  })
}

module.exports = {
  esbuildBundle,
}
