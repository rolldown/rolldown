import { defineConfig } from 'rolldown'
import { babel } from '@rollup/plugin-babel'
import reactRefresh from './plugin-react-refresh/index.cjs'

export default defineConfig({
  input: './main.js',
  plugins: [
    reactRefresh(),
    babel({
      extensions: ['.js', '.jsx', ''],
      include: ['/@react-refresh', '*.js', '*.jsx'],
      babelHelpers: 'inline',
      skipPreflightCheck: true,
      plugins: ['@babel/plugin-transform-modules-commonjs'],
    }),
  ],
  output: {
    format: 'app',
  },
})
