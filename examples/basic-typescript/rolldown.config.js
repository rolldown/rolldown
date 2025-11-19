import { defineConfig } from 'rolldown';
import { viteAssetImportMetaUrlPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  plugins: [
    viteAssetImportMetaUrlPlugin({ clientEntry: '' }),
  ],
});
