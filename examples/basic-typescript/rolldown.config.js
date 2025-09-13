import { defineConfig } from 'rolldown';
import { esmExternalRequirePlugin } from 'rolldown/experimental';

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  external: ['lodash', /abc/],
  plugins: [
    esmExternalRequirePlugin({
      external: ['lodash', /abc/],
    }),
  ],
});
