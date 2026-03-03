import type { InputOptions } from 'rolldown';
import { defineTest } from 'rolldown-tests';

const pluginA = {
  name: 'nested-plugin-1',
  options(options: InputOptions) {
    // @ts-expect-error
    options.plugins!.push(pluginB);
  },
  transform(code: string) {
    return code.replace('foo = 1', 'foo = 2');
  },
};

const pluginB = Promise.resolve({
  name: 'async-plugin-2',
  transform(code: string) {
    return code.replace('answer = 41', 'answer = 42');
  },
});

module.exports = defineTest({
  config: {
    // oxlint-disable-next-line no-sparse-arrays
    plugins: [[Promise.resolve(pluginA)], [undefined, Promise.resolve([null])], ,],
  },
  afterTest: (_output) => {
    import('./assert.mjs');
  },
});
