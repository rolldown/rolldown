globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

const unrelated = [false, 0];
unrelated[1] = 1;
const custom = [false];
custom.extra = 1;
const deleted = [false, 1];
delete deleted[1];
const grown = [false];
grown[3] = 1;

export function main() {
  if (unrelated[0]) log('DEAD_unrelated_write');
  if (custom[0]) log('DEAD_custom_write');
  if (deleted[0]) log('DEAD_delete');
  if (grown[0]) log('DEAD_past_end_write');
  return 1;
}
