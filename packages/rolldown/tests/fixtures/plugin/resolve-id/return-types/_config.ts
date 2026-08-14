// Rolldown should able to recognize kind of return types of the `resolveId` plugin hook.

import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect, vi } from 'vitest';

const entry = path.join(__dirname, './main.js');

const returnNull = vi.fn();
const returnUndefined = vi.fn();
const returnString = vi.fn();
const returnObjId = vi.fn();

export default defineTest({
  sequential: true,
  config: {
    input: entry,
    plugins: [
      {
        name: 'return-null',
        resolveId() {
          returnNull();
          return null;
        },
      },
      {
        name: 'return-undefined',
        resolveId() {
          returnUndefined();
          return null;
        },
      },
      {
        name: 'return-string',
        resolveId(id) {
          if (id === 'foo') {
            returnString();
            return path.resolve(__dirname, './foo.js');
          }
          return null;
        },
      },
      {
        name: 'return-obj-id',
        resolveId(id) {
          if (id === 'bar') {
            returnObjId();
            return {
              id: path.resolve(__dirname, './bar.js'),
            };
          }
        },
      },
    ],
  },
  afterTest: () => {
    expect(returnNull).toBeCalledTimes(3);
    expect(returnUndefined).toBeCalledTimes(3);
    expect(returnString).toHaveBeenCalledOnce();
    expect(returnObjId).toHaveBeenCalledOnce();
  },
});
