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
const convert = (msg) => ({
  valueOf() {
    log(msg);
    return 1;
  },
  toString() {
    log(msg);
    return 'x';
  },
});

// Arguments that are not primitives run code, and arguments out of range throw.
'abc'.at(convert('LIVE_string_argument'));
'a'.concat(convert('LIVE_concat_argument'));
'x'.replace(
  {
    [Symbol.replace]() {
      log('LIVE_symbol_replace');
      return '';
    },
  },
  'y',
);
'x'.replace('x', () => {
  log('LIVE_replace_callback');
  return 'y';
});
/x/.test(convert('LIVE_regexp_argument'));
attempt('LIVE_radix_throw', () => {
  (123).toString(1);
});
attempt('LIVE_to_fixed_throw', () => {
  (123).toFixed(101);
});
attempt('LIVE_repeat_throw', () => {
  'x'.repeat(-1);
});
attempt('LIVE_symbol_throw', () => {
  Symbol({
    toString() {
      throw new Error('description');
    },
  });
});

// A method that the object defines, on itself or on its prototype.
const ownMethod = {
  hasOwnProperty() {
    log('LIVE_own_method');
    return true;
  },
};
ownMethod.hasOwnProperty('x');
const protoMethod = {
  __proto__: {
    hasOwnProperty() {
      log('LIVE_proto_method');
      return false;
    },
  },
};
protoMethod.hasOwnProperty('x');

// A write to a frozen object throws. `Object.freeze` is shallow.
const frozen = { a: 1, nested: { a: 1 } };
Object.freeze(frozen);
attempt('LIVE_frozen_write', () => {
  frozen.a = 2;
});
attempt('LIVE_frozen_add', () => {
  frozen.b = 2;
});
attempt('LIVE_frozen_delete', () => {
  delete frozen.a;
});
frozen.nested.a = 2;
if (frozen.nested.a === 2) log('LIVE_freeze_shallow');
const frozenArray = [1];
Object.freeze(frozenArray);
attempt('LIVE_frozen_push', () => {
  frozenArray.push(2);
});
const frozenRead = { a: 'LIVE_frozen_read' };
Object.freeze(frozenRead);
log(frozenRead.a);

// The state of a regular expression object.
const re = /x/g;
re.test('x');
if (re.lastIndex === 1) log('LIVE_regexp_state');

export const done = true;
