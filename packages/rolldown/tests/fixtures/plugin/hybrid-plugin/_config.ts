import { expect } from 'vitest'
import path from 'node:path'
import { defineTest } from '@tests'
import { loadFallbackPlugin } from 'rolldown/experimental'
import { RolldownPlugin } from '@src/plugin'

const entry = path.join(__dirname, './main.js')

function removeConsoleForPathWithQuery(): RolldownPlugin[] {
  return [
    loadFallbackPlugin(),
    {
      name: 'remove-console',
      transform(code) {
        return code.replace('console.log', '')
      },
    },
  ]
}
export default defineTest({
  config: {
    input: entry,
    plugins: [
      {
        name: 'test-plugin',
        banner: () => '/* Lorem ipsum */',
      },
      removeConsoleForPathWithQuery(),
    ],
  },
  afterTest: async (output) => {
    expect(output.output[0].code).toContain('/* Lorem ipsum */')
    await import('./assert.mjs')
  },
})
