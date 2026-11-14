import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const sourcemapPaths: string[] = [];

export default defineTest({
  sequential: true,
  config: {
    output: {
      dir: 'dist',
      entryFileNames: 'chunks/[name].js',
      sourcemap: true,
      sourcemapFileNames: 'maps/[name].map',
      sourcemapPathTransform: (source, sourcemapPath) => {
        sourcemapPaths.push(sourcemapPath);
        return source;
      },
    },
  },
  afterTest: () => {
    expect(sourcemapPaths).toStrictEqual([
      path.join(import.meta.dirname, 'dist/chunks/main.js.map'),
    ]);
  },
});
