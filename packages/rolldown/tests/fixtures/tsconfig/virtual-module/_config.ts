import { defineTest } from 'rolldown-tests';

const virtual = 'virtual:module';

export default defineTest({
  config: {
    input: {
      main: virtual,
    },
    tsconfig: './tsconfig.json',
    plugins: [
      {
        name: 'virtual-module',
        resolveId(source) {
          if (source === virtual) {
            return source;
          }
        },
        load(id) {
          if (id === virtual) {
            return `export { default } from "@/index";`;
          }
        },
      },
    ],
  },
  afterTest: async () => {
    await import('./assert.mjs');
  },
});
