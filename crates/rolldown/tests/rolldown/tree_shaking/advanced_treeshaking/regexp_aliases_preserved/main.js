globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A global or sticky expression changes `lastIndex`, which code reads.
const global = /a/g;
global.test('a');
if (global.lastIndex === 1) log('LIVE_test_last_index');
const sticky = /a/y;
sticky.exec('a');
if (sticky.lastIndex === 1) log('LIVE_exec_last_index');
const holder = { pattern: /a/g };
holder.pattern.test('a');
if (holder.pattern.lastIndex === 1) log('LIVE_holder_last_index');
const original = /a/g;
const alias = original;
alias.test('a');
if (original.lastIndex === 1) log('LIVE_alias_last_index');
const exported = /a/g;
exported.test('a');
export function lastIndex() {
  return exported.lastIndex;
}

// A write can set `exec`, which `test` calls.
const replaced = /a/;
replaced.exec = () => {
  log('LIVE_own_exec');
  return null;
};
replaced.test('a');

// A conversion of `lastIndex` or of an argument runs code.
const converted = /a/g;
converted.lastIndex = {
  valueOf() {
    log('LIVE_last_index_value_of');
    return 0;
  },
};
converted.test('a');
const argument = /a/;
argument.test({
  toString() {
    log('LIVE_test_argument');
    return 'a';
  },
});

// Other code can read an expression that escapes.
const escaped = /a/g;
globalThis.escapedPattern = escaped;
escaped.test('a');
if (globalThis.escapedPattern.lastIndex === 1) log('LIVE_escaped_last_index');
delete globalThis.escapedPattern;

export const done = true;
