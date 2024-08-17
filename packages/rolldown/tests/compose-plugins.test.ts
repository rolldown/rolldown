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

  it('should compose `banner`, `footer`, `intro`, `outro` hooks', () => {
    const plugins: Plugin[] = [
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
    ]
    const composed = composePlugins(plugins)
    expect(composed.length).toBe(1)
  })
})
