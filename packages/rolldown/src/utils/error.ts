import { type BindingError } from '../binding';
import type { RollupError } from '../log/logging';

function normalizeBindingError(e: BindingError): Error {
  return e.type === 'JsError'
    ? e.field0
    : Object.assign(new Error(), {
      kind: e.field0.kind,
      message: e.field0.message,
      stack: undefined,
    });
}

export function aggregateBindingErrorsIntoError(
  rawErrors: BindingError[],
): Error {
  const errors = rawErrors.map(normalizeBindingError);

  // based on https://github.com/evanw/esbuild/blob/9eca46464ed5615cb36a3beb3f7a7b9a8ffbe7cf/lib/shared/common.ts#L1673
  // combine error messages as a top level error
  let summary = `Build failed with ${errors.length} error${
    errors.length < 2 ? '' : 's'
  }:\n`;
  for (let i = 0; i < errors.length; i++) {
    summary += '\n';
    if (i >= 5) {
      summary += '...';
      break;
    }
    summary += getErrorMessage(errors[i]);
  }
  const wrapper = new Error(summary);
  // expose individual errors as getters so that
  // `console.error(wrapper)` doesn't expand unnecessary details
  // when they are already presented in `wrapper.message`
  Object.defineProperty(wrapper, 'errors', {
    configurable: true,
    enumerable: true,
    get: () => errors,
    set: (value) =>
      Object.defineProperty(wrapper, 'errors', {
        configurable: true,
        enumerable: true,
        value,
      }),
  });
  return wrapper;
}

function getErrorMessage(e: RollupError): string {
  // If the `kind` field is present, we assume it represents
  // a custom error defined by rolldown on the Rust side.
  if (Object.hasOwn(e, 'kind')) {
    return e.message;
  }

  let s = '';
  if (e.plugin) {
    s += `[plugin ${e.plugin}]`;
  }
  const id = e.id ?? e.loc?.file;
  if (id) {
    s += ' ' + id;
    if (e.loc) {
      s += `:${e.loc.line}:${e.loc.column}`;
    }
  }
  if (s) {
    s += '\n';
  }
  const message = `${e.name ?? 'Error'}: ${e.message}`;
  s += message;
  if (e.frame) {
    s = joinNewLine(s, e.frame);
  }
  // copy stack since it's important for js plugin error
  if (e.stack) {
    s = joinNewLine(s, e.stack.replace(message, ''));
  }
  if (e.cause) {
    s = joinNewLine(s, 'Caused by:');
    s = joinNewLine(
      s,
      getErrorMessage(e.cause as any).split('\n').map(line => '  ' + line).join(
        '\n',
      ),
    );
  }
  return s;
}

function joinNewLine(s1: string, s2: string): string {
  // ensure single new line in between
  return s1.replace(/\n+$/, '') + '\n' + s2.replace(/^\n+/, '');
}
