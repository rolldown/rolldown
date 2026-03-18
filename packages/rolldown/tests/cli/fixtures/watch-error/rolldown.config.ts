import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'index.ts',
  cwd: import.meta.dirname,
  watch: {
    watcher: {
      usePolling: true,
    },
  },
});
