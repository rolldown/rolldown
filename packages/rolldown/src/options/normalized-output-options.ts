import type { OutputOptions } from '../types/output-options'

export type InternalModuleFormat = 'es' | 'cjs' | 'iife' | 'umd'

export interface NormalizedOutputOptions extends OutputOptions {}
