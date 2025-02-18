import { defineConfig } from 'rolldown'

export default defineConfig({
  input: 'index.js',
  plugins: [
    {
      name: 'test-input-plugin',
      options: function (options) {
        console.log('input cli default options', options)
      },
      outputOptions: function (outputOptions) {
        console.log('output cli default options', outputOptions)
      },
    }
  ]
})
