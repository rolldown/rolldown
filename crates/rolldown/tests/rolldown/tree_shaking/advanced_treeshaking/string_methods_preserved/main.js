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

// An argument that is not valid throws.
attempt('LIVE_repeat_negative', () => {
  'a'.repeat(-1);
});
attempt('LIVE_repeat_infinity', () => {
  'a'.repeat(Infinity);
});
attempt('LIVE_repeat_length', () => {
  'ab'.repeat(2 ** 29);
});
attempt('LIVE_pad_start_length', () => {
  'a'.padStart(2 ** 30);
});
attempt('LIVE_pad_end_length', () => {
  'a'.padEnd(2 ** 30, 'xy');
});
attempt('LIVE_normalize_form', () => {
  'a'.normalize('bad');
});
attempt('LIVE_match_pattern', () => {
  'a'.match('(');
});
attempt('LIVE_search_pattern', () => {
  'a'.search('[');
});
attempt('LIVE_match_all_flags', () => {
  'a'.matchAll(/a/);
});
attempt('LIVE_match_all_pattern', () => {
  'a'.matchAll('(');
});
attempt('LIVE_locale_compare_tag', () => {
  'a'.localeCompare('b', 'not a tag');
});
attempt('LIVE_locale_lower_tag', () => {
  'a'.toLocaleLowerCase('not a tag');
});
attempt('LIVE_locale_upper_tag', () => {
  'a'.toLocaleUpperCase('-');
});

// A conversion of an argument runs code.
'a'.repeat({
  valueOf() {
    log('LIVE_repeat_count');
    return 1;
  },
});
'a'.padStart(3, {
  toString() {
    log('LIVE_pad_fill');
    return 'x';
  },
});
'a'.normalize({
  toString() {
    log('LIVE_normalize_to_string');
    return 'NFC';
  },
});
'a'.match({
  [Symbol.match]() {
    log('LIVE_custom_matcher');
    return null;
  },
});
'a'.search({
  [Symbol.search]() {
    log('LIVE_custom_searcher');
    return -1;
  },
});

// A sticky or global expression changes `lastIndex`.
const sticky = /a/y;
'a'.match(sticky);
if (sticky.lastIndex === 1) log('LIVE_match_last_index');
const global = /a/g;
global.lastIndex = 1;
'aa'.match(global);
if (global.lastIndex === 0) log('LIVE_match_reset_last_index');

export const done = true;
