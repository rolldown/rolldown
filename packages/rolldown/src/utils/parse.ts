import type { Program } from '@oxc-project/types';
import {
  parse as originalParse,
  type ParseResult as BindingParseResult,
  type ParserOptions as BindingParserOptions,
  parseSync as originalParseSync,
} from '../binding.cjs';
import { runWithRuntimeLease } from './run-with-runtime-lease';
import { shouldEagerlyFreeOutputs } from './threadless-free';
// @ts-ignore
import * as oxcParserWrap from 'oxc-parser/src-js/wrap.js';

/**
 * Result of parsing a code
 *
 * @category Utilities
 */
export interface ParseResult extends BindingParseResult {}
/**
 * Options for parsing a code
 *
 * @category Utilities
 */
export interface ParserOptions extends BindingParserOptions {}

/**
 * Wrap a native `ParseResult` for consumers.
 *
 * Lazy flavors get oxc-parser's own wrap object: its getters read -- and
 * thereby drain, every upstream getter is a `mem::take` -- each native field
 * on first access, and finalizers release whatever stays unread.
 *
 * The threadless-WASI flavor never runs finalizers (see
 * `src/utils/threadless-free.ts`), and the upstream napi class has no
 * `dropInner`, so reading all four getters immediately is the only way to
 * release the native storage; anything left unread would stay allocated
 * forever (the serialized-AST JSON behind `program` is the largest piece).
 * This snapshot keeps the JSON.parse of the program exactly as lazy and
 * memoized as the wrap object's, via the same `jsonParseAst` revival path.
 */
function wrapParseResult(result: BindingParseResult): ParseResult {
  if (!shouldEagerlyFreeOutputs()) {
    return oxcParserWrap.wrap(result);
  }
  // The native `program` getter returns the serialized AST JSON string (a
  // `mem::take` drain), despite the declared `Program` type.
  const programJson = result.program as unknown as string;
  const module = result.module;
  const comments = result.comments;
  const errors = result.errors;
  let program: Program | undefined;
  return {
    get program() {
      if (!program) program = oxcParserWrap.jsonParseAst(programJson) as Program;
      return program;
    },
    get module() {
      return module;
    },
    get comments() {
      return comments;
    },
    get errors() {
      return errors;
    },
  };
}

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
 * Generally {@linkcode parseSync} is preferable to use as it does not have the overhead of spawning a thread.
 * If you need to parallelize parsing multiple files, it is recommended to use worker threads.
 *
 * @category Utilities
 */
export async function parse(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): Promise<ParseResult> {
  return wrapParseResult(
    await runWithRuntimeLease(
      () => originalParse(filename, sourceText, options),
      'Parse and runtime release both failed',
    ),
  );
}

/**
 * Parse JS/TS source synchronously on current thread.
 *
 * This is generally preferable over {@linkcode parse} (async) as it does not have the overhead
 * of spawning a thread, and the majority of the workload cannot be parallelized anyway
 * (see {@linkcode parse} documentation for details).
 *
 * If you need to parallelize parsing multiple files, it is recommended to use worker threads
 * with {@linkcode parseSync} rather than using {@linkcode parse}.
 *
 * @category Utilities
 */
export function parseSync(
  filename: string,
  sourceText: string,
  options?: ParserOptions | null,
): ParseResult {
  return wrapParseResult(originalParseSync(filename, sourceText, options));
}
