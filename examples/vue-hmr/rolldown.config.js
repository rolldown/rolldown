import { defineConfig } from 'rolldown'
import vuePlugin from '@vitejs/plugin-vue'

export default defineConfig({
  input: './main.js',
  define: {
    'process.env.NODE_ENV': '"development"',
  },
  plugins: [
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
