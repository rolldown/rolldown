export function bar() {
  import('./dead-module');
}

// These should NOT be optimized away — the import result is consumed
export async function baz() {
  console.log(await import('./used-module-1'));
}

export function qux() {
  window.foo(import('./used-module-2'));
}
