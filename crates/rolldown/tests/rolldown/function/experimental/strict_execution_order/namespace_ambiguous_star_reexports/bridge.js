export function ok() {
  return 1;
}

// `collision` is ambiguous and therefore absent from this module's namespace.
export * from './common-a.js';
export * from './common-b.js';
