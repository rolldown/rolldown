import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const transformFn = vi.fn();

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform: function (code, id, meta) {
          const isFooJS = id.endsWith('foo.js');
          transformFn(isFooJS);
          if (isFooJS) {
            expect(code).toStrictEqual('');
            expect(meta.moduleType).toEqual('js');
            return {
              code: `console.log('transformed')`,
            };
          }
        },
      },
    ],
  },
  afterTest(output) {
    expect(transformFn).toHaveBeenCalledTimes(3);
    expect(transformFn.mock.calls.filter((args) => args[0] === true).length).toBe(1);
    expect(output.output[0].code).contains('transformed');
  },
});
