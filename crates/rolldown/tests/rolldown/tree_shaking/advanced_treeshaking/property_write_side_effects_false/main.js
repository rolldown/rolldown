globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// The analysis runs with `propertyWriteSideEffects: false`.
const o = { used: 1, dead: 'DEAD_property' };
export function main() {
  return o.used;
}

// The inclusion engine includes a property write only with its object. The write still
// changes the facts.
const cfg = { debug: false };
cfg.debug = true;
if (cfg.debug) log('LIVE_gated_write');
export const done = true;
