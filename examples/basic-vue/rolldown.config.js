import { defineConfig } from 'rolldown';

export default defineConfig({
  input: './index.ts',
  transform: {
    typescript: {
      onlyRemoveTypeImports: true,
    },
  },
});
