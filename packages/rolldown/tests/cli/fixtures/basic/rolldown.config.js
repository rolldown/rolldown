import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default {
  // input: 'src/index.js',
  input: path.resolve(__dirname, 'src/index.js'),
  output: [
    {
      // dir: 'build',
      // file: 'build/bundle.js',
      dir: path.resolve(__dirname, 'build'),
      file: path.resolve(__dirname, 'build/bundle.js'),
    },
  ],
  resolve: {
    conditionNames: ['import'],
    alias: {
      // modules: 'src/modules',
      modules: path.resolve(__dirname, 'src/modules'),
    },
  },
}
