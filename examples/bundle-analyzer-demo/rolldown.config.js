import { defineConfig } from 'rolldown';
import { bundleAnalyzerPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    main: './src/main.js',
    worker: './src/worker.js',
  },
  output: {
    dir: 'dist',
    format: 'esm',
    entryFileNames: '[name]-[hash].js',
    chunkFileNames: 'chunks/[name]-[hash].js',
    // Force utils into a single shared chunk to demonstrate optimization suggestions
    manualChunks(id) {
      if (id.includes('/utils/') || id.includes('\\utils\\')) {
        return 'utils';
      }
    },
  },
  plugins: [
    bundleAnalyzerPlugin({
      format: 'md',
    }),
  ],
});
