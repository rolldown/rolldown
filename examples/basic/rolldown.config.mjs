import { defineConfig } from 'rolldown'
import { importGlobPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: ['./main.js'],
  plugins: [
    importGlobPlugin({
      // root: import.meta.dirname,
      // restoreQueryExtension: false,
    }),
  ],
})
