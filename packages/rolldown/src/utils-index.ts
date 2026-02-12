export { parse, parseSync, type ParseResult, type ParserOptions } from './utils/parse';
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
