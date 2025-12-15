import {
  parse as originalParse,
  type ParseResult,
  type ParserOptions,
  parseSync as originalParseSync,
} from '../binding.cjs';
// @ts-ignore
import * as oxcParserWrap from 'oxc-parser/src-js/wrap.js';

/**
 * Parse asynchronously.
 *
 * Note: This function can be slower than `parseSync` due to the overhead of spawning a thread.
 */
export async function parse(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): Promise<ParseResult> {
  return oxcParserWrap.wrap(await originalParse(filename, sourceText, options));
}

/** Parse synchronously. */
export function parseSync(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): ParseResult {
  return oxcParserWrap.wrap(originalParseSync(filename, sourceText, options));
}
