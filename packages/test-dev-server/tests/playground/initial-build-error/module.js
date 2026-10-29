// This module ships BROKEN: the unterminated string on the last line makes the
// playground's FIRST build fail, so the harness serves the spinner + error
// overlay without any server being created in the spec. The spec rewrites that
// last line to a slot comment to make the module valid and assert recovery.
export const value = 'initial-build-error: ok';

document.querySelector('.app').textContent = value;

const broken = '
