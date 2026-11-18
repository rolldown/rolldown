import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    experimental: { importAttributes: true },
    plugins: [
      {
        name: 'test',
        resolveId(source, _importer, { attributes }) {
          if (source.includes('logo.png')) {
            return { id: source, attributes };
          }
        },
        load(id) {
          if (id.includes('logo.png')) {
            return `export default ${JSON.stringify(id)}`;
          }
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    const { smallLogoImage, largeLogoImage } = await import('./dist/main.js');
    expect(smallLogoImage).toBe('./logo.png?attributes=%7B%22size%22%3A%22100%22%7D');
    expect(largeLogoImage).toBe('./logo.png?attributes=%7B%22size%22%3A%22200%22%7D');
  },
});
