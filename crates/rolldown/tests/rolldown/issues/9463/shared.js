export function foo() {
  (globalThis.sideEffectLog ??= []).push('shared-foo');
}
