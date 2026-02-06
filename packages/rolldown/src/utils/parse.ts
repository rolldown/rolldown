import {
  parse as originalParse,
  type ParseResult,
  type ParserOptions,
  parseSync as originalParseSync,
} from '../binding.cjs';
// @ts-ignore
import * as oxcParserWrap from 'oxc-parser/src-js/wrap.js';

export type { ParseResult, ParserOptions };

/**
 * Parse JS/TS source asynchronously on a separate thread.
 *
 * Note that not all of the workload can happen on a separate thread.
 * Parsing on Rust side does happen in a separate thread, but deserialization of the AST to JS objects
 * has to happen on current thread. This synchronous deserialization work typically outweighs
 * the asynchronous parsing by a factor of between 3 and 20.
 *
 * i.e. the majority of the workload cannot be parallelized by using this method.
 *
 * Generally `parseSync` is preferable to use as it does not have the overhead of spawning a thread.
 * If you need to parallelize parsing multiple files, it is recommended to use worker threads.
 */
export async function parse(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): Promise<ParseResult> {
  return oxcParserWrap.wrap(await originalParse(filename, sourceText, options));
}

/**
 * Parse JS/TS source synchronously on current thread.
 *
 * This is generally preferable over `parse` (async) as it does not have the overhead
 * of spawning a thread, and the majority of the workload cannot be parallelized anyway
 * (see `parse` documentation for details).
 *
 * If you need to parallelize parsing multiple files, it is recommended to use worker threads
 * with `parseSync` rather than using `parse`.
 */
export function parseSync(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): ParseResult {
  return oxcParserWrap.wrap(originalParseSync(filename, sourceText, options));
}
