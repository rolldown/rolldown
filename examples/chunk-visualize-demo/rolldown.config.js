import { defineConfig } from 'rolldown';
import { chunkVisualizePlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    main: './src/main.js',
    worker: './src/worker.js',
  },
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [chunkVisualizePlugin()],
});
