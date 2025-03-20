import { Program } from '@oxc-project/types'
import { parseSync, parseAsync } from './binding'
import type { ParseResult, ParserOptions } from './binding'
import { locate } from './log/locate-character'
import { error, logParseError } from './log/logs'
import { getCodeFrame } from './utils/code-frame'
// @ts-ignore
import * as oxcParserWrap from "oxc-parser/wrap.mjs"

function wrap(result: ParseResult, sourceText: string) {
  // reuse oxc-parser wrap and eagerly throw an error if any
  result = oxcParserWrap.wrap(result);
  if (result.errors.length > 0) {
    return normalizeParseError(sourceText, result.errors)
  }
  return result.program
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

const defaultParserOptions: ParserOptions = {
  lang: 'js',
  preserveParens: false,
}

// The api compat to rollup `parseAst` and `parseAstAsync`.

export function parseAst(
  sourceText: string,
  options?: ParserOptions | undefined | null,
  filename?: string,
): Program {
  return wrap(
    parseSync(filename ?? 'file.js', sourceText, {
      ...defaultParserOptions,
      ...options,
    }),
    sourceText,
  )
}

export async function parseAstAsync(
  sourceText: string,
  options?: ParserOptions | undefined | null,
  filename?: string,
): Promise<Program> {
  return wrap(
    await parseAsync(filename ?? 'file.js', sourceText, {
      ...defaultParserOptions,
      ...options,
    }),
    sourceText,
  )
}

export type { ParseResult, ParserOptions }
