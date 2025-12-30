import type { BindingRenderedChunk } from '../../dist/binding.cjs';
import type { AssetSource } from '../utils/asset-source';
import type { ExternalMemoryHandle } from './external-memory-handle';

/** @category Plugin APIs */
export interface OutputAsset extends ExternalMemoryHandle {
  type: 'asset';
  fileName: string;
  /** @deprecated Use "originalFileNames" instead. */
  originalFileName: string | null;
  originalFileNames: string[];
  source: AssetSource;
  /** @deprecated Use "names" instead. */
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
  x_google_ignoreList?: number[];
  toString(): string;
  toUrl(): string;
}

/** @category Plugin APIs */
export interface RenderedModule {
  readonly code: string | null;
  renderedLength: number;
  renderedExports: string[];
}

/** @category Plugin APIs */
export interface RenderedChunk extends Omit<BindingRenderedChunk, 'modules'> {
  type: 'chunk';
  modules: {
    [id: string]: RenderedModule;
  };
  name: string;
  isEntry: boolean;
  isDynamicEntry: boolean;
  facadeModuleId: string | null;
  moduleIds: Array<string>;
  exports: Array<string>;
  fileName: string;
  imports: Array<string>;
  dynamicImports: Array<string>;
}

/** @category Plugin APIs */
export interface OutputChunk extends ExternalMemoryHandle {
  type: 'chunk';
  code: string;
  name: string;
  isEntry: boolean;
  exports: string[];
  fileName: string;
  modules: {
    [id: string]: RenderedModule;
  };
  imports: string[];
  dynamicImports: string[];
  facadeModuleId: string | null;
  isDynamicEntry: boolean;
  moduleIds: string[];
  map: SourceMap | null;
  sourcemapFileName: string | null;
  preliminaryFileName: string;
}

/** @category Programmatic APIs */
export interface RolldownOutput extends ExternalMemoryHandle {
  output: [OutputChunk, ...(OutputChunk | OutputAsset)[]];
}
