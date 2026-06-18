import { stripVTControlCharacters as strip } from 'node:util';
import type { RollupError } from 'rolldown';

/**
 * Serializable build-error payload sent to the browser, mirroring the `err`
 * field of Vite's `ErrorPayload` (`packages/vite/types/hmrPayload.d.ts`).
 */
export interface PreparedError {
  message: string;
  stack: string;
  id?: string;
  frame?: string;
  plugin?: string;
  pluginCode?: string;
  loc?: {
    file?: string;
    line: number;
    column: number;
  };
}

/**
 * Port of Vite's `prepareError` (`server/middlewares/error.ts`): copy only the
 * fields the client overlay needs and strip ANSI control characters so they
 * don't leak into the browser. Avoids serializing the whole error object, since
 * some errors attach large objects (e.g. PostCSS).
 */
export function prepareError(err: Error | RollupError): PreparedError {
  const rollupErr = err as RollupError;
  return {
    message: strip(err.message),
    stack: strip(cleanStack(err.stack || '')),
    id: rollupErr.id,
    frame: strip(rollupErr.frame || ''),
    plugin: rollupErr.plugin,
    pluginCode: rollupErr.pluginCode?.toString(),
    loc: rollupErr.loc,
  };
}

function cleanStack(stack: string): string {
  return stack
    .split(/\n/)
    .filter((l) => /^\s*at/.test(l))
    .join('\n');
}
