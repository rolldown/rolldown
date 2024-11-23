import type { OutputOptions } from '../options/output-options'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd'

export interface NormalizedOutputOptions extends OutputOptions {}
