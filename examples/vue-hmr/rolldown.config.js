import { defineConfig } from 'rolldown'
// The plugin-vue-rolldown-hmr-demo copy form @vitejs/plugin-vue, it save descriptor cache make sure `_rerender_only` work at rolldown.
import vuePlugin from 'plugin-vue-rolldown-hmr-demo'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [
    // Add option to vue plugin inject hmr related code
    vuePlugin({
      devServer: {
        watcher: {
          on() {},
        },
        config: {
          server: {
            hmr: true,
          },
        },
      },
    }),
    {
      name: 'emit-html',
      generateBundle() {
        this.emitFile({
          type: 'asset',
          fileName: 'index.html',
          source: `<div id="app"></div><script src="./main.js"></script>`,
        })
      },
    },
  ],
  output: {
    format: 'app',
  },
  dev: true,
})
