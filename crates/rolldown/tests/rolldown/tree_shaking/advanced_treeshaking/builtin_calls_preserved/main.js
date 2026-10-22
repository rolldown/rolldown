globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);
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
try {
  (123).toString(1);
} catch {
  log('LIVE_radix_throw');
}
try {
  (123).toFixed(101);
} catch {
  log('LIVE_to_fixed_throw');
}
try {
  'x'.repeat(-1);
} catch {
  log('LIVE_repeat_throw');
}
try {
  Symbol({
    toString() {
      throw new Error('description');
    },
  });
} catch {
  log('LIVE_symbol_throw');
}

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
try {
  frozen.a = 2;
} catch {
  log('LIVE_frozen_write');
}
try {
  frozen.b = 2;
} catch {
  log('LIVE_frozen_add');
}
try {
  delete frozen.a;
} catch {
  log('LIVE_frozen_delete');
}
frozen.nested.a = 2;
if (frozen.nested.a === 2) log('LIVE_freeze_shallow');
const frozenArray = [1];
Object.freeze(frozenArray);
try {
  frozenArray.push(2);
} catch {
  log('LIVE_frozen_push');
}
const frozenRead = { a: 'LIVE_frozen_read' };
Object.freeze(frozenRead);
log(frozenRead.a);

// The state of a regular expression object.
const re = /x/g;
re.test('x');
if (re.lastIndex === 1) log('LIVE_regexp_state');

export const done = true;
