import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'index.js',
  platform: 'node',
  inject: {
    require:
      `data:application/javascript;pltaintext,import { createRequire } from 'module'; export const require = createRequire(import.meta.url);`,
  },
  plugins: [
    {
      name: 'test',
      resolveId(id) {
        console.log(`id: `, id);
      },
      transform(code, id) {
        console.log(`id: `, id);
        console.log(code);
      },
    },
  ],
});
