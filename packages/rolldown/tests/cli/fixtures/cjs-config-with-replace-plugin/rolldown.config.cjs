const rolldown = require('rolldown')
const plugin = require('rolldown/experimental')

module.exports = rolldown.defineConfig({
  input: './index.js',
  plugins: [
    plugin.replacePlugin({
      '__rolldown': '1'
    }),
  ],
})
