import init from './add.wasm?init'

init().then(({ exports }) => {
  exports.add(1, 2) === 3
})
