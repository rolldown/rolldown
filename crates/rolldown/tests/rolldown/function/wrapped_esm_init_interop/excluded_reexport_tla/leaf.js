globalThis.__log.push('LEAF:before');
await Promise.resolve();
globalThis.__ready = 'ready';
globalThis.__log.push('LEAF:after');

export const used = 'used';
export const unused = 'unused';
