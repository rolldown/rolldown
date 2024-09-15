import { defineConfig } from 'rolldown'
import reactRefresh from './plugin-react-refresh/index.cjs'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [reactRefresh()],
  output: {
    format: 'app',
  },
  dev: true,
})
