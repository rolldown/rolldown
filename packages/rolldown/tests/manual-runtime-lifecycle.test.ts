import { expect, test } from 'vitest';

import { acquireRuntimeLease, isRuntimeLeaseRequired } from '../src/runtime-lifecycle';
import { getRuntimeCapabilitiesCompat } from '../src/runtime-support';

const capabilities = getRuntimeCapabilitiesCompat();

test.runIf(capabilities.target !== 'wasi-threads')(
  'runtime leases remain no-ops outside threaded WASI',
  async () => {
    expect(isRuntimeLeaseRequired()).toBe(false);

    const lease = await acquireRuntimeLease();
    expect(() => lease.release()).not.toThrow();
    expect(() => lease.release()).not.toThrow();
  },
);
