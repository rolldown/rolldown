const pattern = /DEAD_test/;
const exec = /DEAD_exec/;
const string = /DEAD_to_string/;
const global = /DEAD_global/g;

export function main() {
  pattern.test('a');
  exec.exec('a');
  string.toString();
  global.test('a');
  return 1;
}
