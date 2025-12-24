// Test case: re-export from CJS module with namespace alias
// This creates a namespace_alias for the "default" export
// Combined with helper.js importing from the same CJS module,
// the namespace refs get merged
export { default as CJS } from 'this-is-only-used-for-testing';
export { useFoo } from './helper.js';
