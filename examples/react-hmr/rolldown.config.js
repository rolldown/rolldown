import { defineConfig } from 'rolldown'
import { reactPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [reactPlugin()],
  output: {
    format: 'app',
  },
  dev: true,
})
