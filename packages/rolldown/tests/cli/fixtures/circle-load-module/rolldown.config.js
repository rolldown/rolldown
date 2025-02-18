import { defineConfig } from 'rolldown';

export default defineConfig({
  input: ['./index.js'],
  plugins: [
    {
      name: 'foo',
      async transform(code, id) {
        if (id.includes('bar.mjs')) {
          console.log('transform bar');
          const res = await this.resolve('./bar.mjs');
          await this.load(res);
          console.log('transformed bar');
        } else if (id.includes('bar2.mjs')) {
          console.log('transform bar2');
          const res = await this.resolve('./bar.mjs');
          await this.load(res);
          console.log('transformed bar2');
        }
      },
    },
  ],
});


