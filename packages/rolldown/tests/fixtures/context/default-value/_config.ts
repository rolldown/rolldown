import path from 'node:path'
import { expect } from 'vitest'
import { defineTest } from 'rolldown-tests'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'plugin-for-test',
        options(opt) {
          expect(opt.context).toBe(undefined);
        },
        renderStart(_, inputOptions) {
          // When option is normalized, the default value of context should be string "undefined";
          expect(inputOptions.context).toBe("undefined");
        }
      },
    ],
  },
})
