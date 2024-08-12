import init from './add.wasm'

init().then(({ exports }) => {
  exports.add(1, 2) === 3
})
