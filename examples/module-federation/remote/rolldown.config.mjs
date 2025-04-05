import { defineConfig } from 'rolldown';
import { moduleFederationPlugin } from 'rolldown/experimental';

// TODO: can't resolve `./Button.jsx` at ubuntu.
export default defineConfig({
  input: './button.jsx',
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
      manifest: true,
      getPublicPath: 'http://localhost:8085/',
    }),
  ],
});
