import { defineTest } from '@tests'
import { stripVTControlCharacters } from 'util'
import { assert, expect } from 'vitest'

export default defineTest({
  config: {
    input: ['main1.js', 'main2.js'],
  },
  catchError(e) {
    assert(e instanceof AggregateError)
    for (let error of e.errors) {
      error.message = stripVTControlCharacters(error.message)
    }
    expect(e.errors).toMatchObject([
      {
        kind: 'PARSE_ERROR',
        message: expect.stringContaining('invalid :('),
      },
      {
        kind: 'PARSE_ERROR',
        message: expect.stringContaining('invalid :)'),
      },
    ])
    expect(e.message).toBe('Build failed')
  },
})
