import { test, expect, describe } from 'vitest'
import { scan } from 'rolldown/experimental'

describe('experimental_scan', () => {
  test('call options hook', async () => {
    expect.assertions(1)

    await scan({
      input: 'virtual',
      plugins: [
        {
          name: 'test',
          options(opts) {
            expect(opts).toBeTruthy()
          },
          resolveId(id) {
            if (id === 'virtual') return '\0' + id
          },
          load(id) {
            if (id === '\0virtual') return ''
          }
        }
      ]
    })
  })
})
