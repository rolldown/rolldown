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

// A later write to a frozen object throws.
const frozenArray = Object.freeze([1]);
attempt('LIVE_frozen_array_write', () => {
  frozenArray[0] = 2;
});
const frozenFunction = Object.freeze(function () {});
attempt('LIVE_frozen_function_write', () => {
  frozenFunction.extra = 1;
});
const frozenObject = { value: 1 };
Object.freeze(frozenObject);
attempt('LIVE_frozen_object_write', () => {
  frozenObject.value = 2;
});
const frozenPush = Object.freeze([]);
attempt('LIVE_frozen_push', () => {
  frozenPush.push(1);
});

// `Object.values` and `Object.entries` run getters and throw for `null` and `undefined`.
Object.values({
  get value() {
    log('LIVE_values_getter');
    return 1;
  },
});
Object.entries({
  get value() {
    log('LIVE_entries_getter');
    return 1;
  },
});
attempt('LIVE_values_null', () => {
  Object.values(null);
});
attempt('LIVE_entries_undefined', () => {
  Object.entries(undefined);
});
const defined = {};
Object.defineProperty(defined, 'value', {
  enumerable: true,
  get() {
    log('LIVE_defined_getter');
    return 1;
  },
});
Object.values(defined);
const proxied = new Proxy(
  { value: 1 },
  {
    ownKeys(target) {
      log('LIVE_proxy_keys');
      return Reflect.ownKeys(target);
    },
  },
);
Object.entries(proxied);

export const done = true;
