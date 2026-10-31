import type { BindingRenderedChunk } from '../binding.cjs';
import type { AssetSource } from '../utils/asset-source';
import type { ExternalMemoryHandle } from './external-memory-handle';

/**
 * The information about an asset in the generated bundle.
 *
 * @category Plugin APIs
 */
export interface OutputAsset extends ExternalMemoryHandle {
  type: 'asset';
  /** The file name of this asset. */
  fileName: string;
  /** @deprecated Use {@linkcode originalFileNames} instead. */
  originalFileName: string | null;
  /** The list of the absolute paths to the original file of this asset. */
  originalFileNames: string[];
  /** The content of this asset. */
  source: AssetSource;
  /** @deprecated Use {@linkcode names} instead. */
  name: string | undefined;
  names: string[];
}

/** @category Plugin APIs */
export interface SourceMap {
  file: string;
  mappings: string;
  names: string[];
  sources: string[];
  sourcesContent: string[];
  version: number;
  debugId?: string;
  ignoreList?: number[];
  /** @deprecated Use `ignoreList` instead. */
  x_google_ignoreList?: number[];
  toString(): string;
  toUrl(): string;
}

/** @category Plugin APIs */
export interface RenderedModule {
  /**
   * The code fragment assigned to this module during chunk rendering, or `null`
   * if the module has no rendered code.
   *
   * When chunk optimization runs before module rendering, this is the optimized
   * fragment. Inlining and statement merging can move code between modules or
   * into generated chunk code. This fragment does not represent all code
   * originating from the module and is not necessarily independently executable.
   * Later chunk transformations are not reflected in this value.
   */
  readonly code: string | null;
  /**
   * The UTF-16 length of {@linkcode code}, or `0` when `code` is `null`.
   * This measures the assigned fragment, not the module's original contribution
   * to the chunk after cross-module optimization.
   */
  renderedLength: number;
  /**
   * The list of exported names from this module.
   *
   * The names that are not used are not included. These are the module's
   * original export names retained by linking and tree shaking, even if chunk
   * optimization subsequently inlines their code into another fragment.
   */
  renderedExports: string[];
}

/**
 * The information about the chunk being rendered.
 *
 * Unlike {@link OutputChunk}, `code` and `map` are not set as the chunk has not been rendered yet.
 * All referenced chunk file names in each property that would contain hashes will contain hash placeholders instead.
 *
 * @category Plugin APIs
 */
export interface RenderedChunk extends Omit<BindingRenderedChunk, 'modules'> {
  type: 'chunk';
  /** Information about the modules included in this chunk. */
  modules: {
    [id: string]: RenderedModule;
  };
  /** The name of this chunk, which is used in naming patterns. */
  name: string;
  /** Whether this chunk is a static entry point. */
  isEntry: boolean;
  /** Whether this chunk is a dynamic entry point. */
  isDynamicEntry: boolean;
  /** The id of a module that this chunk corresponds to. */
  facadeModuleId: string | null;
  /** The list of ids of modules included in this chunk. */
  moduleIds: Array<string>;
  /** Exported variable names from this chunk. */
  exports: Array<string>;
  /** The preliminary file name of this chunk with hash placeholders. */
  fileName: string;
  /** External modules imported statically by this chunk. */
  imports: Array<string>;
  /** External modules imported dynamically by this chunk. */
  dynamicImports: Array<string>;
}

/**
 * The information about a chunk in the generated bundle.
 *
 * @category Plugin APIs
 */
export interface OutputChunk extends ExternalMemoryHandle {
  type: 'chunk';
  /** The generated code of this chunk. */
  code: string;
  /** The name of this chunk, which is used in naming patterns. */
  name: string;
  /** Whether this chunk is a static entry point. */
  isEntry: boolean;
  /** Exported variable names from this chunk. */
  exports: string[];
  /** The file name of this chunk. */
  fileName: string;
  /** Information about the modules included in this chunk. */
  modules: {
    [id: string]: RenderedModule;
  };
  /** External modules imported statically by this chunk. */
  imports: string[];
  /** External modules imported dynamically by this chunk. */
  dynamicImports: string[];
  /** The id of a module that this chunk corresponds to. */
  facadeModuleId: string | null;
  /** Whether this chunk is a dynamic entry point. */
  isDynamicEntry: boolean;
  moduleIds: string[];
  /** The source map of this chunk if present. */
  map: SourceMap | null;
  sourcemapFileName: string | null;
  /** The preliminary file name of this chunk with hash placeholders. */
  preliminaryFileName: string;
}

/**
 * The generated bundle output.
 *
 * @category Programmatic APIs
 */
export interface RolldownOutput extends ExternalMemoryHandle {
  /**
   * The list of chunks and assets in the generated bundle.
   *
   * This includes at least one {@linkcode OutputChunk}. It may also include more
   * {@linkcode OutputChunk} and/or {@linkcode OutputAsset}s.
   */
  output: [OutputChunk, ...(OutputChunk | OutputAsset)[]];
  /**
   * The updated property-name cache. Present when `minify.mangleProps` runs.
   *
   * This contains the input cache and any generated mappings. Pass it to a later
   * `minify.mangleProps.cache` to reuse the same mappings.
   */
  mangleCache?: Record<string, string | false>;
}
