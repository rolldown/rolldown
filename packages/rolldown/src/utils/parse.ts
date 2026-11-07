import type { Program } from '@oxc-project/types';
import {
  parse as originalParse,
  type ParseResult as BindingParseResult,
  type ParserOptions as BindingParserOptions,
  parseSync as originalParseSync,
} from '../binding.cjs';
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
 * Wrap a native `ParseResult` for consumers. Lazy flavors use oxc-parser's
 * wrap object, whose getters drain each native field on first access.
 * A threadless WASI host may never run GC finalizers and the napi class has no
 * `dropInner`, so all four getters are read at once; `program` is still revived
 * lazily through the same `jsonParseAst` path.
 */
function wrapParseResult(result: BindingParseResult): ParseResult {
  if (!shouldEagerlyFreeOutputs()) {
    return oxcParserWrap.wrap(result);
  }
  // The native `program` getter returns the serialized AST JSON string (a
  // `mem::take` drain), despite the declared `Program` type.
  let programJson: string | undefined = result.program as unknown as string;
  const module = result.module;
  const comments = result.comments;
  const errors = result.errors;
  let program: Program | undefined;
  let revived = false;
  return {
    get program() {
      if (!revived) {
        program = oxcParserWrap.jsonParseAst(programJson) as Program;
        // Drop the JSON once revived, so a kept result holds only the AST.
        // Only after `jsonParseAst` returns (a throw keeps the string for a
        // retry); the memo keys on `revived`, not on a truthy AST.
        revived = true;
        programJson = undefined;
      }
      return program as Program;
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
  return wrapParseResult(await originalParse(filename, sourceText, options));
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
