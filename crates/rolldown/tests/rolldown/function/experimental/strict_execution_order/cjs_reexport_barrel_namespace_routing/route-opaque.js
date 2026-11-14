import * as ns from 'namespace-barrel';

globalThis.__events.push('route-opaque');

export const value = ns[globalThis.__namespaceKey];
