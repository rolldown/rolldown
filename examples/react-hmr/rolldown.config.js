import { defineConfig } from 'rolldown'
import { babel } from '@rollup/plugin-babel'
import reactRefresh from './plugin-react-refresh/index.cjs'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [
    reactRefresh(),
    babel({
      extensions: ['.js', '.jsx', ''],
      include: ['/@react-refresh', /\.jsx?$/],
      exclude: /node_modules/,
      babelHelpers: 'inline',
      skipPreflightCheck: true,
      plugins: ['@babel/plugin-transform-modules-commonjs'],
    }),
  ],
  output: {
    format: 'app',
  },
})
