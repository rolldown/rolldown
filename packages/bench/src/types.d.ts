import type { BuildOptions } from 'esbuild';
import type { OutputOptions, RolldownOptions } from 'rolldown';
import type {
  OutputOptions as RollupOutputOptions,
  RollupOptions,
} from 'rollup';

type BundlerName = 'rolldown' | 'rollup' | 'esbuild';

type RolldownOptionsWithSingleOutput = Omit<RolldownOptions, 'output'> & {
  output?: RolldownOutputOptions;
};

type RollupOptionsWithSingleOutput = Omit<RollupOptions, 'output'> & {
  output?: RollupOutputOptions;
};

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
    | RolldownOptionsWithSingleOutput
    | { name: string; options: RolldownOptionsWithSingleOutput }[];
  rollupOptions?: RollupOptionsWithSingleOutput;
  esbuildOptions?: BuildOptions;
}

export interface RolldownBenchSuite {
  suiteName: string;
  title: string;
  inputs: string[];
  options?: RolldownOptionsWithSingleOutput;
}
