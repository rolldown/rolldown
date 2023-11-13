const b = require('benny')
const chalk = require('chalk')
const { rolldownBundle } = require('./rolldown.cjs')
const { esbuildBundle } = require('./esbuild.cjs')

const sleep = function () {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve()
    }, 1000)
  })
}

module.exports = b.suite(
  'Suite two',

  b.add('rolldown/three.js', async () => {
    await rolldownBundle(['../temp/three.js/src/Three.js'])
  }),

  b.add('esbuild/three.js', async () => {
    await esbuildBundle()
  }),
  b.cycle(),
  b.complete(),
)
