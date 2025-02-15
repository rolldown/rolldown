import { Program } from '@oxc-project/types'
import { parseSync, parseAsync } from './binding'
import type { ParseResult, ParserOptions } from './binding'
import { locate } from './log/locate-character'
import { error, logParseError } from './log/logs'
import { getCodeFrame } from './utils/code-frame'

// The oxc program is a string in the result, we need to parse it to a object.
// Copy from https://github.com/oxc-project/oxc/blob/main/napi/parser/index.js#L12
function wrap(result: ParseResult, sourceText: string) {
  let program: ParseResult['program'],
    module: ParseResult['module'],
    comments: ParseResult['comments'],
    errors: ParseResult['errors'],
    magicString: ParseResult['magicString']
  return {
    get program() {
      if (!errors) errors = result.errors
      if (errors.length > 0) {
        return normalizeParseError(sourceText, errors)
      }
      // @ts-expect-error the result.program typing is `Program`
      if (!program) program = JSON.parse(result.program)
      return program
    },
    get module() {
      if (!module) module = result.module
      return module
    },
    get comments() {
      if (!comments) comments = result.comments
      return comments
    },
    get errors() {
      if (!errors) errors = result.errors
      return errors
    },
    get magicString() {
      if (!magicString) magicString = result.magicString
      return magicString
    },
  }
}

function normalizeParseError(
  sourceText: string,
  errors: ParseResult['errors'],
) {
  let message = `Parse failed with ${errors.length} error${errors.length < 2 ? '' : 's'}:\n`
  for (let i = 0; i < errors.length; i++) {
    if (i >= 5) {
      message += '\n...'
      break
    }
    const e = errors[i]
    message +=
      e.message +
      '\n' +
      e.labels
        .map((label: any) => {
          const location = locate(sourceText, label.start, { offsetLine: 1 })
          if (!location) {
            return
          }
          return getCodeFrame(sourceText, location.line, location.column)
        })
        .filter(Boolean)
        .join('\n')
  }
  return error(logParseError(message))
}

// The api compat to rollup `parseAst` and `parseAstAsync`.

export function parseAst(
  filename: string,
  sourceText: string,
  options?: ParserOptions | undefined | null,
): Program {
  return wrap(parseSync(filename, sourceText, options), sourceText).program
}

export async function parseAstAsync(
  filename: string,
  sourceText: string,
  options?: ParserOptions | undefined | null,
): Promise<Program> {
  return wrap(await parseAsync(filename, sourceText, options), sourceText)
    .program
}

export type { ParseResult, ParserOptions }
