import { defineTest } from 'rolldown-tests'
import { stripVTControlCharacters } from 'util'
import { expect } from 'vitest'

export default defineTest({
  config: {
    input: ['main1.js', 'main2.js'],
  },
  catchError(e: any) {
    e.message = stripVTControlCharacters(e.message)
    for (let error of e.errors) {
      error.message = stripVTControlCharacters(error.message)
    }
    // top level summary
    expect(e.message).toContain('Build failed with 2 errors')
    expect(e.message).toContain('invalid :(')
    expect(e.message).toContain('invalid :)')
    // diagnostics
    expect(e.errors).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          kind: 'PARSE_ERROR',
          message: expect.stringContaining('invalid :('),
        }),
        expect.objectContaining({
          kind: 'PARSE_ERROR',
          message: expect.stringContaining('invalid :)'),
        }),
      ]),
    )
  },
})
