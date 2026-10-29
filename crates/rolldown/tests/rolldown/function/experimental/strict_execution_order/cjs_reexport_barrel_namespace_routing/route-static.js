import * as ns from 'namespace-barrel';

globalThis.__events.push('route-static');

export const value = new ns.Stack().size;
