import { expect, describe, test, vi, afterEach } from 'vitest'

afterEach(() => {
  vi.resetModules()
})

describe('brandColor', () => {
  describe('use color', () => {
    test('ansi true color', async () => {
      vi.doMock('../../src/cli/env.js', () => ({
        isColorSupported: true,
        colorDepth: 24,
      }))

      const { brandColor } = await import('../../src/cli/colors.js')
      expect(brandColor('rolldown')).toBe(
        '\u001b[38;2;227;151;9mrolldown\u001b[39m',
      )
    })

    test('ansi 256 color', async () => {
      vi.doMock('../../src/cli/env.js', () => ({
        isColorSupported: true,
        colorDepth: 8,
      }))

      const { brandColor } = await import('../../src/cli/colors.js')
      expect(brandColor('rolldown')).toBe('\u001b[38;5;178mrolldown\u001b[39m')
    })

    test('ansi 16 color', async () => {
      vi.doMock('../../src/cli/env.js', () => ({
        isColorSupported: true,
        colorDepth: 4,
      }))

      const { brandColor } = await import('../../src/cli/colors.js')
      expect(brandColor('rolldown')).toBe('\u001b[33mrolldown\u001b[39m')
    })

    test('less than 4 color', async () => {
      vi.doMock('../../src/cli/env.js', () => ({
        isColorSupported: true,
        colorDepth: 3,
      }))

      const { brandColor } = await import('../../src/cli/colors.js')
      expect(brandColor('rolldown')).toBe('rolldown')
    })
  })

  test('not use color', async () => {
    vi.doMock('../../src/cli/env.js', () => ({
      isColorSupported: false,
    }))

    const { brandColor } = await import('../../src/cli/colors.js')
    expect(brandColor('rolldown')).toBe('rolldown')
  })
})
