import fsExtra from 'fs-extra'
// setup three-js

if (fsExtra.existsSync('./tmp/bench/multi-duplicated-symbol')) {
  console.log('[skip] setup multi-duplicated-symbol already')
} else {
  console.log('Setup `multi-duplicated-symbol` in tmp/bench')
  fsExtra.copySync(
    './tmp/github/multi-duplicated-symbol',
    `./tmp/bench/multi-duplicated-symbol`,
  )
}
