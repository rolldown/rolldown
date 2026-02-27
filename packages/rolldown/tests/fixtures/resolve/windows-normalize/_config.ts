import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: [path.join(__dirname, './main.js').replaceAll('\\', '/')],
    resolve: {
      symlinks: false,
    },
  },
  afterTest(output) {
    function countOccurrences(str: string, substr: string) {
      let count = 0;
      let index = str.indexOf(substr);
      while (index !== -1) {
        count++;
        index = str.indexOf(substr, index + 1);
      }
      return count;
    }

    expect(countOccurrences(output.output[0].code, 'console.log')).toBe(1);
  },
});
