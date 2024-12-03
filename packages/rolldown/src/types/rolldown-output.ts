import { AssetSource } from '../utils/asset-source'
import type { OutputAsset, OutputChunk } from '../rollup'
import type { HasProperty, IsPropertiesEqual, TypeAssert } from './assert'
import type { RenderedChunk } from '../binding'

export interface RolldownOutputAsset {
  type: 'asset'
  fileName: string
  /** @deprecated Use "originalFileNames" instead. */
  originalFileName: string | null
  originalFileNames: string[]
  source: AssetSource
  /** @deprecated Use "names" instead. */
  name: string | undefined
  names: string[]
}

function _assertRolldownOutputAsset() {
  type _ = TypeAssert<IsPropertiesEqual<RolldownOutputAsset, OutputAsset>>
}

export interface SourceMap {
  file: string
  mappings: string
  names: string[]
  sources: string[]
  sourcesContent: string[]
  version: number
  // TODO
  // toString(): string
  // toUrl(): string
}

export interface RolldownRenderedModule {
  readonly code: string | null
  renderedLength: number
}

export interface RolldownRenderedChunk extends Omit<RenderedChunk, 'modules'> {
  modules: {
    [id: string]: RolldownRenderedModule
  }
}

export interface RolldownOutputChunk {
  type: 'chunk'
  code: string
  name: string
  isEntry: boolean
  exports: string[]
  fileName: string
  modules: {
    [id: string]: RolldownRenderedModule
  }
  imports: string[]
  dynamicImports: string[]
  facadeModuleId: string | null
  isDynamicEntry: boolean
  moduleIds: string[]
  map: SourceMap | null
  sourcemapFileName: string | null
  preliminaryFileName: string
}

function _assertRolldownOutputChunk() {
  type _ = TypeAssert<
    IsPropertiesEqual<Omit<RolldownOutputChunk, 'modules' | 'map'>, OutputChunk>
  >
}

export interface RolldownOutput {
  output: [
    RolldownOutputChunk,
    ...(RolldownOutputChunk | RolldownOutputAsset)[],
  ]
}

function _assertRolldownOutput() {
  type _ = TypeAssert<HasProperty<RolldownOutput, 'output'>>
}
