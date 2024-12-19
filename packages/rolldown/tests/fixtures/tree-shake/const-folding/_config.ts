import type { OutputChunk as RolldownOutputChunk } from 'rolldown'
import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    plugins: [
      {
        name: 'replace NODE_ENV',
        transform: (code, id) => {
          const res = code.replace(
            'process.env.NODE_ENV',
            JSON.stringify('development'),
          )
          return res
        },
      },
    ],
  },
  afterTest: (output) => {
    output.output
      .filter(({ type }) => type === 'chunk')
      .forEach((chunk) => {
        let code = (chunk as RolldownOutputChunk).code
        expect(code.includes(`production`)).toBe(false)
        expect(code.includes(`development`)).toBe(true)
      })
  },
})
