import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    external: (source: string, importer: string | undefined) => {
      expect(importer).toStrictEqual(path.join(__dirname, 'main.js'));
      if (source.startsWith('external')) {
        return true;
      }
    },
  },
});
