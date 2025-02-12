import { defineConfig } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './index.jsx',
  plugins: [
    moduleFederationPlugin({
      name: 'mf-host',
      remotes: {
        button: {
          name: 'button',
          type: 'module',
          entry: 'http://localhost:8085/remote-entry.js',
        },
      },
      shared: {
        react: {
          singleton: true,
        },
      },
    }),
    {
      name: 'emit-html',
      generateBundle() {
        const html = `
          <html>
            <body>
              <div id="app"></div>
              <script type="module" src="./index.js"></script>
            </body>
          </html>
        `
        this.emitFile({
          type: 'asset',
          fileName: 'index.html',
          source: html,
        })
      },
    },
  ],
})
