globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A closure sees a later write, and a local function writes a top-level binding.
export function closure() {
  const o = { used: false };
  function read() {
    return o.used;
  }
  o.used = true;
  return read();
}
if (closure()) log('LIVE_closure_write');
function makeCounter() {
  let n = 0;
  return {
    inc() {
      n++;
      return n;
    },
  };
}
const counter = makeCounter();
counter.inc();
if (counter.inc() === 2) log('LIVE_closure_counter');
let topFlag = false;
function localSet() {
  function set() {
    topFlag = true;
  }
  set();
}
localSet();
if (topFlag) log('LIVE_local_set');

// Destructuring runs getters, also through a rest element that no code reads.
const withGetter = {
  get used() {
    log('LIVE_destructure_getter');
    return 1;
  },
};
const { used } = withGetter;
const withRestGetter = {
  used: 1,
  get other() {
    log('LIVE_rest_getter');
    return 2;
  },
};
const { used: restUsed, ...rest } = withRestGetter;
function restParam({ used, ...rest }) {
  return used;
}
restParam({
  used: 1,
  get other() {
    log('LIVE_rest_param_getter');
    return 2;
  },
});
const nestedRest = {
  inner: {
    used: 1,
    get other() {
      log('LIVE_nested_rest_getter');
      return 2;
    },
  },
};
const {
  inner: { used: innerUsed, ...innerRest },
} = nestedRest;
let assignedUsed, assignedRest;
({ used: assignedUsed, ...assignedRest } = {
  used: 1,
  get other() {
    log('LIVE_assignment_rest_getter');
    return 2;
  },
});

// Code reads the copy of a rest element, a pattern sees a write, and a default runs.
const copied = { used: 1, other: 'LIVE_rest_copy' };
const { used: copiedUsed, ...copy } = copied;
log(copy.other);
const changed = { nested: { flag: false } };
changed.nested.flag = true;
const {
  nested: { flag },
} = changed;
if (flag) log('LIVE_pattern_after_write');
const withDefault = { a: undefined };
const { a = 'LIVE_default_value' } = withDefault;
log(a);

export const done = true;
