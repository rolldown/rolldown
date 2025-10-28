import type { TransformOptions as OxcTransformOptions } from '../binding.cjs';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';
import {
  logDeprecatedDefine,
  logDeprecatedDropLabels,
  logDeprecatedInject,
} from '../log/logs';
import type { InputOptions } from '../options/input-options';

interface NormalizedTransformOptions {
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
export function normalizeTransformOptions(
  inputOptions: InputOptions,
  onLog: LogHandler,
): NormalizedTransformOptions {
  const transform = inputOptions.transform;

  // Extract define - prefer transform.define over top-level define
  let define: Array<[string, string]> | undefined;
  if (transform?.define) {
    define = Object.entries(transform.define);
  } else if (inputOptions.define) {
    // Warn about deprecated top-level define
    onLog(LOG_LEVEL_WARN, logDeprecatedDefine());
    define = Object.entries(inputOptions.define);
  }

  // Extract inject - prefer transform.inject over top-level inject
  let inject: Record<string, string | [string, string]> | undefined;
  if (transform?.inject) {
    inject = transform.inject;
  } else if (inputOptions.inject) {
    // Warn about deprecated top-level inject
    onLog(LOG_LEVEL_WARN, logDeprecatedInject());
    inject = inputOptions.inject;
  }

  // Extract dropLabels - prefer transform.dropLabels over top-level dropLabels
  let dropLabels: string[] | undefined;
  if (transform?.dropLabels) {
    dropLabels = transform.dropLabels;
  } else if (inputOptions.dropLabels) {
    // Warn about deprecated top-level dropLabels
    onLog(LOG_LEVEL_WARN, logDeprecatedDropLabels());
    dropLabels = inputOptions.dropLabels;
  }

  // Extract OXC transform options (excluding define, inject, and dropLabels)
  let oxcTransformOptions: OxcTransformOptions | undefined;
  if (transform) {
    const {
      define: _define,
      inject: _inject,
      dropLabels: _dropLabels,
      ...rest
    } = transform;
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
