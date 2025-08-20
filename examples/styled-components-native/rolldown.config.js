import { defineConfig } from 'rolldown';

export default defineConfig({
  input: './src/index.jsx',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  transform: {
    plugins: {
      styledComponents: {
        displayName: true,
        fileName: true,
        ssr: true,
        transpileTemplateLiterals: true,
        minify: true,
        pure: true,
        namespace: 'rolldown-example',
      },
    },
  },
});
