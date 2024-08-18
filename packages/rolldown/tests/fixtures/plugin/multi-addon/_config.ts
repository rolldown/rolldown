import { expect } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin-1',
        banner: '/*banner1*/',
      },
      {
        name: 'test-plugin-2',
        banner: () => '/*banner2*/',
      },
      {
        name: 'test-plugin-3',
        footer: '/*footer1*/',
      },
      {
        name: 'test-plugin-4',
        footer: () => '/*footer2*/',
      },
      {
        name: 'test-plugin-5',
        intro: '/*intro1*/',
      },
      {
        name: 'test-plugin-6',
        intro: () => '/*intro2*/',
      },
      {
        name: 'test-plugin-7',
        outro: '/*outro1*/',
      },
      {
        name: 'test-plugin-8',
        outro: () => '/*outro2*/',
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatch(/\/\*banner1\*\/\n\/\*banner2\*\//)
    expect(output.output[0].code).toMatch(/\/\*footer1\*\/\n\/\*footer2\*\//)
    expect(output.output[0].code).toMatch(/\/\*intro1\*\/\n\/\*intro2\*\//)
    expect(output.output[0].code).toMatch(/\/\*outro1\*\/\n\/\*outro2\*\//)
  },
})
