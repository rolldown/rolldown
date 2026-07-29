import { expect, test } from 'vitest';

import { acquireRuntimeLease, isRuntimeLeaseRequired } from '../src/runtime-lifecycle';

// Every current binding reports the shared backend, so runtime leases are
// no-ops on every target; only legacy tokio-backed threaded-WASI bindings
// (synthesized by the compat shim) ever lease.
test('runtime leases are no-ops for every current binding', async () => {
  expect(isRuntimeLeaseRequired()).toBe(false);

  const lease = await acquireRuntimeLease();
  expect(() => lease.release()).not.toThrow();
  expect(() => lease.release()).not.toThrow();
});
