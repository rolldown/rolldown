import { defineTest } from 'rolldown-tests';
import { getOutputSourcemapFilenames } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      sourcemap: true,
      sourcemapFileNames: (chunk) => {
        expect(chunk).toHaveProperty('name');
        if (chunk.name === 'emitted') {
          return '[name]-[chunkhash].js.map';
        }
        if (chunk.name === 'nested') return 'folder/[name].js.map';
        return '1-[name].js.map';
      },
    },
  },
  afterTest: (output) => {
    const mainFile = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName.startsWith('emitted'),
    )!;
    const hash = mainFile.fileName.match(/^emitted-(\w{8})\.js$/)![1];
    expect(getOutputSourcemapFilenames(output)).toStrictEqual([
      '1-main.js.map',
      `emitted-${hash}.js.map`,
      'folder/nested.js.map',
    ]);
  },
});
