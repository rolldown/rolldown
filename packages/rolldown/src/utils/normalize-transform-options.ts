import type { TransformOptions as OxcTransformOptions } from '../binding';
import type { LogHandler } from '../log/log-handler';
import { LOG_LEVEL_WARN } from '../log/logging';
import { logDeprecatedDefine, logDeprecatedInject } from '../log/logs';
import type { InputOptions } from '../options/input-options';

interface NormalizedTransformOptions {
  define: Array<[string, string]> | undefined;
  inject: Record<string, string | [string, string]> | undefined;
  oxcTransformOptions: OxcTransformOptions | undefined;
}

/**
 * Normalizes transform options by extracting `define` and `inject` separately from OXC transform options.
 *
 * Prioritizes values from `transform.define` and `transform.inject` over deprecated top-level options.
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

  // Extract OXC transform options (excluding define and inject)
  let oxcTransformOptions: OxcTransformOptions | undefined;
  if (transform) {
    const { define: _define, inject: _inject, ...rest } = transform;
    // Only set oxcTransformOptions if there are actual options
    if (Object.keys(rest).length > 0) {
      oxcTransformOptions = rest as OxcTransformOptions;
    }
  }

  return {
    define,
    inject,
    oxcTransformOptions,
  };
}
