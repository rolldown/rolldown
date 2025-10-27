import type { BuildOptions } from 'esbuild';
import type { RolldownOptions } from 'rolldown';
import type { RollupOptions } from 'rollup';

type BundlerName = 'rolldown' | 'rollup' | 'esbuild';

export interface BenchSuite {
  derived?: {
    // Whether to have an extra round for benchmarking with enabling sourcemap
    sourcemap?: boolean;
    minify?: boolean;
  };
  title: string;
  inputs: string[];
  disableBundler?: BundlerName | BundlerName[];
  // Multiple rolldown options will result in multiple runs with different options.
  // This is useful for benchmarking different options with the same input in rolldown.
  rolldownOptions?:
    | RolldownOptions
    | { name: string; options: RolldownOptions }[];
  rollupOptions?: RollupOptions;
  esbuildOptions?: BuildOptions;
}

export interface RolldownBenchSuite {
  suiteName: string;
  title: string;
  inputs: string[];
  options?: RolldownOptions;
}
