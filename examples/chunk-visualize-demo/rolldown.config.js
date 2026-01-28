import { defineConfig } from 'rolldown';
import { chunkVisualizePlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    main: './src/main.js',
  },
  output: {
    dir: 'dist',
    format: 'esm',
    entryFileNames: '[name]-[hash].js',
    chunkFileNames: 'chunks/[name]-[hash].js',
  },
  plugins: [
    // Enable chunk visualization to generate analyze-data.json
    chunkVisualizePlugin(),
    // Or with custom filename:
    // chunkVisualizePlugin({
    //   fileName: 'bundle-analysis.json'
    // })
  ],
});
