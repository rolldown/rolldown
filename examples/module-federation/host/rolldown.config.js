import { defineConfig } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './index.jsx',
  plugins: [
    moduleFederationPlugin({
      name: 'mf-host',
      remotes: {
        '@button': 'http://localhost:5176/dist/remote-entry.js',
      },
      shared: {
        react: {
          singleton: true,
        },
      },
    }),
  ],
})
