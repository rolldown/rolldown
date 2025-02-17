import { defineConfig } from 'rolldown'

export default defineConfig({
  input: 'index.js',
  output: [
    {
      format: 'esm',
      entryFileNames: 'esm.js',
    },
    {
      format: 'cjs',
      entryFileNames: 'cjs.js',
    },
  ],
  plugins: [
    {
      options: function () {
        console.log('called options hook')
      },
      outputOptions: function () {
        console.log('called output options hook')
      },
    },
  ],
})
