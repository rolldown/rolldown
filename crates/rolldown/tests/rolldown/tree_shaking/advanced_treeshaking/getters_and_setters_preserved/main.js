globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);
// The alias hides the callback from the `try` rule, so the walk of the callback checks it.
const attempt = (name, run) => {
  const call = run;
  try {
    call();
  } catch {
    log(name);
  }
};

// A getter that writes through `this`, and a getter that writes a binding that code reads.
const viaThis = {
  flag: false,
  get on() {
    this.flag = true;
    return 1;
  },
};
viaThis.on;
if (viaThis.flag) log('LIVE_getter_this');
let seen = false;
const writes = {
  get value() {
    seen = true;
    return 1;
  },
};
writes.value;
if (seen) log('LIVE_getter_binding_write');

// A getter whose value changes.
let state = false;
const changing = {
  get value() {
    return state;
  },
};
state = true;
if (changing.value) log('LIVE_getter_state');

// A getter and a setter that throw.
const throws = {
  get value() {
    throw new Error('LIVE_getter_throw');
  },
  set value(v) {
    throw new Error('LIVE_setter_throw');
  },
};
attempt('LIVE_getter_throw', () => {
  throws.value;
});
attempt('LIVE_setter_throw', () => {
  throws.value = 1;
});

// A setter that writes a binding that code reads, and a setter that writes through `this`.
let setSeen = false;
const setter = {
  flag: false,
  set value(v) {
    setSeen = v;
  },
  set self(v) {
    this.flag = v;
  },
};
setter.value = true;
if (setSeen) log('LIVE_setter_write');
setter.self = true;
if (setter.flag) log('LIVE_setter_this');

// A write to a getter-only property throws.
const getterOnly = {
  get value() {
    return 1;
  },
};
attempt('LIVE_getter_only_write', () => {
  getterOnly.value = 2;
});

// The last definition of a key wins.
const overwritten = {
  get a() {
    log('NEVER_overwritten_getter');
    return 1;
  },
  a: 'LIVE_overwrite_value',
  b: 1,
  get b() {
    log('LIVE_overwrite_getter');
    return 2;
  },
};
log(overwritten.a);
overwritten.b;

export const done = true;
