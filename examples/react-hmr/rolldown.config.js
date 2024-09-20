import { defineConfig } from 'rolldown'
import { reactPlugin, transformPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [
    // transform jsx to js
    transformPlugin({
      reactRefresh: true, // enable react-refresh
    }),
    reactPlugin(), // load `react-refresh-entry.js` and inject react hmr helpers, eg `$RefreshSig$`
  ],
  output: {
    format: 'app',
  },
  dev: true,
})
