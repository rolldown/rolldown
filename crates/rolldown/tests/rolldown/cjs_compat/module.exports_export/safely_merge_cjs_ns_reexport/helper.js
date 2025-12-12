// This import creates a namespace ref that will be merged with entry.js's namespace ref
import { foo } from 'this-is-only-used-for-testing';

export function useFoo() {
  return foo();
}
