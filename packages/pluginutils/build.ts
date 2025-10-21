import { dts } from 'rolldown-plugin-dts';

// use relative path to avoid circular dependency
import { build } from '../rolldown/src/index';

await build({
  input: './src/index.ts',
  plugins: [dts({
    oxc: true,
  })],
  output: {
    dir: './dist',
    format: 'esm',
    cleanDir: true,
  },
});
