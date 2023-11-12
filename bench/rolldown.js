const rolldown = require('@rolldown/core')

async function rolldownBundle(input) {
  const bundler = await rolldown.rolldown({
    input,
  })
  let res = await bundler.generate()
  return res
}

module.exports = {
  rolldownBundle,
}
