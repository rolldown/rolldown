globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// Symbol keys: a getter, a write, and two different symbols.
const key = Symbol();
const withGetter = {
  get [key]() {
    log('LIVE_symbol_getter');
    return 1;
  },
};
withGetter[key];
const written = Symbol();
const writable = { [written]: false };
writable[written] = true;
if (writable[written]) log('LIVE_symbol_write');
const first = Symbol();
const second = Symbol();
const distinct = { [first]: false };
distinct[second] = true;
if (!distinct[first] && distinct[second]) log('LIVE_symbol_identity');

// A read after a `delete`.
const deleted = { a: 'x' };
delete deleted.a;
if (deleted.a === undefined) log('LIVE_delete_read');

// The first definition of a key decides its position in the key order.
const ordered = { a: 1, b: 2, a: 3 };
log(`LIVE_key_order_${Object.keys(ordered).join('')}`);

export const done = true;
