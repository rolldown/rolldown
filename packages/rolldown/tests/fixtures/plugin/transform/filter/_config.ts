import { defineTest } from 'rolldown-tests'
import { expect, vi } from 'vitest'

const transformFn = vi.fn()
const transformFn2 = vi.fn()
const transformFn3 = vi.fn()

export default defineTest({
  config: {
    plugins: [
      {
        name: 'test-plugin',
        transform: {
          filter: {
            id: {
              include: [/foo\.js$/],
            },
          },
          handler(_, id) {
            transformFn()
            if (id.endsWith('foo.js')) {
              return {
                code: `console.log('transformed')`,
              }
            }
          },
        },
      },
      {
        name: 'test2',
        transform: {
          filter: {
            moduleType: ['js'],
          },
          handler(_) {
            transformFn2()
            return null
          },
        },
      },
      {
        name: 'test3',
        transform: {
          filter: {
            code: {
              include: ['hello'],
            },
          },
          handler(_) {
            transformFn3()
            return null
          },
        },
      },
    ],
  },
  afterTest: () => {
    expect(transformFn).toHaveBeenCalledTimes(1)
    expect(transformFn2).toHaveBeenCalledTimes(2)
    expect(transformFn3).toHaveBeenCalledTimes(0)
  },
})
