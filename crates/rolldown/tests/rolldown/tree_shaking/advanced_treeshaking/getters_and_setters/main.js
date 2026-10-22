globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A read of a getter that only returns a value has no effect.
const pure = {
  get a() {
    return 'DEAD_pure_getter';
  },
};
pure.a;

// A getter that writes only to a binding that nothing reads.
let unused;
const writes = {
  get value() {
    unused = 1;
    return 'DEAD_getter_write';
  },
};
writes.value;

// A write that runs a setter with an empty body.
const setter = {
  set value(v) {},
};
setter.value = 'DEAD_setter';

// The value of a getter folds a branch. A getter with an effect still runs.
const flags = {
  get debug() {
    return false;
  },
};
const loud = {
  get debug() {
    log('getter');
    return false;
  },
};
export function main() {
  if (flags.debug) log('DEAD_getter_branch');
  if (loud.debug) log('DEAD_effect_getter_branch');
  return 1;
}

// The value of a getter decides a dynamic import.
export async function load() {
  if (flags.debug) return import('./dead.js');
  return 1;
}
