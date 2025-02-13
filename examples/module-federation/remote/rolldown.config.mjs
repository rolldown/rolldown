import { defineConfig } from 'rolldown'
import { moduleFederationPlugin } from 'rolldown/experimental'

export default defineConfig({
  input: './Button.jsx',
  cwd: import.meta.dirname,
  plugins: [
    moduleFederationPlugin({
      name: 'mf-remote',
      filename: 'remote-entry.js',
      exposes: {
        './button': './Button.jsx',
      },
      shared: {
        react: {
          singleton: true,
        },
      },
    }),
  ],
})
