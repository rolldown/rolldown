import { parseSync, parseAsync } from './binding'
import type { ParseResult, ParserOptions } from './binding'

// The oxc program is a string in the result, we need to parse it to a object.
// Copy from https://github.com/oxc-project/oxc/blob/main/napi/parser/index.js#L12
function wrap(result: ParseResult) {
  let program: ParseResult['program'],
    module: ParseResult['module'],
    comments: ParseResult['comments'],
    errors: ParseResult['errors'],
    magicString: ParseResult['magicString']
  return {
    get program() {
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

// The api compat to rollup `parseAst` and `parseAstAsync`.

export function parseAst(
  filename: string,
  sourceText: string,
  options?: ParserOptions | undefined | null,
): ParseResult {
  return wrap(parseSync(filename, sourceText, options))
}

export async function parseAstAsync(
  filename: string,
  sourceText: string,
  options?: ParserOptions | undefined | null,
): Promise<ParseResult> {
  return wrap(await parseAsync(filename, sourceText, options))
}

export type { ParseResult, ParserOptions }
