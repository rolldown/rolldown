import { defineConfig } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './index.js',
  plugins: [
    moduleFederationPlugin({
      name: 'mf-remote',
      filename: 'remote-entry.js',
      exposes: {
        './button': './button.jsx',
      },
      shared: {
        react: {
          singleton: true,
        },
      },
    }),
  ],
})
