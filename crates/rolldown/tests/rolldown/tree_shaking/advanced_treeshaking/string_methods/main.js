export function main() {
  'DEAD_repeat'.repeat(2);
  'DEAD_pad_start'.padStart(20, 'x');
  'DEAD_pad_end'.padEnd(20, 'x');
  'DEAD_normalize'.normalize('NFC');
  'DEAD_match'.match(/x/);
  'DEAD_match_string'.match('x');
  'DEAD_match_all'.matchAll(/x/g);
  'DEAD_search'.search(/x/);
  'DEAD_search_string'.search('x');
  'DEAD_locale_compare'.localeCompare('x');
  'DEAD_locale_lower'.toLocaleLowerCase('en');
  'DEAD_locale_upper'.toLocaleUpperCase('en');
  return 1;
}
