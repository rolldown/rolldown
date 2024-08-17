import { Plugin } from 'rolldown'
import { composePlugins } from 'rolldown/experimental'
import { describe, expect, it } from 'vitest'

describe('compose-plugins', () => {
  it('should compose `transform` hooks', () => {
    const plugins: Plugin[] = [
      {
        name: 'test-plugin',
        transform: function () {},
      },
      {
        name: 'test-2',
        transform() {
          return null
        },
      },
    ]

    const composed = composePlugins(plugins)
    expect(composed.length).toBe(1)
  })
})
