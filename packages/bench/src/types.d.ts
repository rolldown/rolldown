import type { RolldownOptions } from 'rolldown'
import type { RollupOptions } from 'rollup'
import type { BuildOptions } from 'esbuild'

type BundlerName = 'rolldown' | 'rollup' | 'esbuild'

export interface BenchSuite {
  derived?: {
    // Whether to have an extra round for benchmarking with enabling sourcemap
    sourcemap?: boolean
  }
  title: string
  inputs: string[]
  disableBundler?: BundlerName | BundlerName[]
  rolldownOptions?: RolldownOptions
  rollupOptions?: RollupOptions
  esbuildOptions?: BuildOptions
}
