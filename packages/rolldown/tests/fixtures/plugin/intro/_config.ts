import { expect, vi } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'

const entry = path.join(__dirname, './main.js')

export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        intro: () => '/* Intro */',
      },
    ],
  },
  afterTest: (output) => {
    expect(output.output[0].code).toContain('/* Intro */')
  },
})
