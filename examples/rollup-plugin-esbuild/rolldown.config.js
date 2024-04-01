import { defineConfig } from 'rolldown'
import esbuild from 'rollup-plugin-esbuild'

export default defineConfig({
  input: './src/main.ts',
  plugins: [
    esbuild({
      loaders: {
        svg: 'dataurl',
      },
    }),
  ],
  resolve: {
    extensions: ['.ts', '.js', '.svg'],
  },
})
