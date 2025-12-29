import type { TransformOptions as OxcTransformOptions } from '../binding.cjs';
import type { InputOptions } from '../options/input-options';

export interface NormalizedTransformOptions {
  define: Array<[string, string]> | undefined;
  inject: Record<string, string | [string, string]> | undefined;
  dropLabels: string[] | undefined;
  oxcTransformOptions: OxcTransformOptions | undefined;
}

/**
 * Normalizes transform options by extracting `define`, `inject`, and `dropLabels` separately from OXC transform options.
 *
 * Prioritizes values from `transform.define`, `transform.inject`, and `transform.dropLabels` over deprecated top-level options.
 */
export function normalizeTransformOptions(inputOptions: InputOptions): NormalizedTransformOptions {
  const transform = inputOptions.transform;

  const define = transform?.define ? Object.entries(transform.define) : undefined;
  const inject = transform?.inject;
  const dropLabels = transform?.dropLabels;

  // Extract OXC transform options (excluding define, inject, and dropLabels)
  let oxcTransformOptions: OxcTransformOptions | undefined;
  if (transform) {
    const { define: _define, inject: _inject, dropLabels: _dropLabels, ...rest } = transform;
    // Only set oxcTransformOptions if there are actual options
    if (Object.keys(rest).length > 0) {
      if (rest.jsx === false) {
        rest.jsx = 'disable' as any;
      }
      oxcTransformOptions = rest as OxcTransformOptions;
    }
  }

  return {
    define,
    inject,
    dropLabels,
    oxcTransformOptions,
  };
}
