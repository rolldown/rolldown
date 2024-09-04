import { defineConfig } from 'rolldown'
import { babel } from '@rollup/plugin-babel'

export default defineConfig({
  input: './main.js',
  output: {
    format: 'app',
    plugins: [
      babel({
        babelHelpers: 'bundled',
        plugins: ['@babel/plugin-transform-modules-commonjs'],
      }),
    ],
  },
})
