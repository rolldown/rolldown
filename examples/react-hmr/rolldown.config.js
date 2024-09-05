import { defineConfig } from 'rolldown'
import { babel } from '@rollup/plugin-babel'
import react from '@vitejs/plugin-react'

export default defineConfig({
  input: './main.js',
  plugins: [
    react(),
    babel({
      extensions: ['.js', '.jsx', ''],
      include: ['/@react-refresh', '*.js', '*.jsx'],
      babelHelpers: 'inline',
      skipPreflightCheck: true,
      presets: ['@babel/preset-react'],
      plugins: ['@babel/plugin-transform-modules-commonjs'],
    }),
  ],
  output: {
    format: 'app',
  },
})
