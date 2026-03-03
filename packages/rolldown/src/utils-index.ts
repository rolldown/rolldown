export { parse, parseSync, type ParseResult, type ParserOptions } from './utils/parse';
export type * as ESTree from '@oxc-project/types';
export { minify, type MinifyOptions, type MinifyResult, minifySync } from './utils/minify';
export {
  transform,
  type TransformOptions,
  type TransformResult,
  transformSync,
  TsconfigCache,
  type TsconfigRawOptions,
  type TsconfigCompilerOptions,
} from './utils/transform';
export { Visitor, type VisitorObject } from './utils/visitor';
