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

// The conversion of an element runs code.
new Uint8Array([
  {
    valueOf() {
      log('LIVE_element_value_of');
      return 1;
    },
  },
]);

// A length that is not valid throws.
attempt('LIVE_negative_length', () => {
  new Uint8Array(-1);
});
attempt('LIVE_large_length', () => {
  new Float64Array(2 ** 53);
});

// A BigInt array does not convert numbers.
attempt('LIVE_bigint_array', () => {
  new BigInt64Array([1]);
});

// A write changes the elements of the source array.
const source = [1];
source[0] = {
  valueOf() {
    log('LIVE_written_source');
    return 1;
  },
};
new Uint8Array(source);

// An array-like object runs its getter.
new Uint8Array({
  get length() {
    log('LIVE_array_like_length');
    return 0;
  },
});

// A local binding can have the name of the constructor.
function shadowed() {
  const Uint8Array = function () {
    log('LIVE_shadowed_constructor');
  };
  new Uint8Array([1]);
}
shadowed();

// The typed array holds the converted values.
const typed = new Uint8Array([257]);
if (typed[0] === 1) log('LIVE_typed_value');

export const done = true;
