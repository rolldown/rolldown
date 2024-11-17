import { defineTest } from '@tests'
import { stripVTControlCharacters } from 'util'
import { assert, expect } from 'vitest'

export default defineTest({
  config: {
    input: ['main1.js', 'main2.js'],
  },
  catchError(e) {
    assert(e instanceof AggregateError)
    e.message = stripVTControlCharacters(e.message)
    for (let error of e.errors) {
      error.message = stripVTControlCharacters(error.message)
    }
    // top level summary
    expect(e.message).toContain('Build failed with 2 errors')
    expect(e.message).toContain('invalid :(')
    expect(e.message).toContain('invalid :)')
    // diagnostics
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
  },
})
