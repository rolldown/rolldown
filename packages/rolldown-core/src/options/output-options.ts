import { OutputOptions as RollupOutputOptions } from '../rollup-types'
import { OutputOptions as BindingOutputOptions } from '@rolldown/node-binding'
import { unimplemented } from '../utils'

export interface OutputOptions extends RollupOutputOptions {
  // --- NotGoingToSupports

  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  amd?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  compact?: never
  // deprecated
  dynamicImportFunction?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  dynamicImportInCjs?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  experimentalMinChunkSize?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  extend?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  externalImportAssertions?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  externalLiveBindings?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  hoistTransitiveImports?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  indent?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  manualChunks?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  minifyInternalExports?: never
  // deprecated
  namespaceToStringTag?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  noConflict?: never
  // deprecated
  preferConst?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  preserveModules?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  preserveModulesRoot?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  sanitizeFileName?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  systemNullSetters?: never
  /**
   * @deprecated
   * TODO: Rolldown might supports this in a long term. Need to investigate.
   */
  validate?: never

  // --- ToBeSupported

  banner?: never
  makeAbsoluteExternalsRelative?: never
  moduleContext?: never
  shimMissingExports?: never
  assetFileNames?: never
  chunkFileNames?: never
  entryFileNames?: never
  esModule?: never
  footer?: never
  freeze?: never
  generatedCode?: never
  globals?: never
  inlineDynamicImports?: never
  intro?: never
  name?: never
  outro?: never
  paths?: never
  plugins?: never
  sourcemap?: never
  sourcemapBaseUrl?: never
  sourcemapExcludeSources?: never
  sourcemapFile?: never
  sourcemapPathTransform?: never
  strict?: never
  interop?: never

  // Rewritten

  file?: never // TODO: Rolldown might supports this in a long term. Need to investigate.
}

function normalizeFormat(
  format: OutputOptions['format'],
): BindingOutputOptions['format'] {
  if (format === 'esm' || format === 'cjs') {
    return format
  } else {
    return unimplemented(`output.format: ${format}`)
  }
}

export function normalizeOutputOptions(
  opts: OutputOptions,
): BindingOutputOptions {
  const { dir, format, exports, ...rest } = opts
  // Make sure all fields of RollupInputOptions are handled.
  // @ts-expect-error
  const _empty: never = undefined as unknown as NonNullable<
    (typeof rest)[keyof typeof rest]
  >
  return {
    dir: dir,
    format: normalizeFormat(format),
    exports,
  }
}
