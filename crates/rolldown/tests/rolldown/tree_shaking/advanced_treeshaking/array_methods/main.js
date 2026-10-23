export function main() {
  ['DEAD_concat'].concat([1]);
  [1].concat(2, 'DEAD_concat_primitive');
  ['DEAD_reduce'].reduce((x, y) => x + y, '');
  ['DEAD_reduce_right'].reduceRight((x, y) => x + y, '');
  ['DEAD_reduce_no_start', 'x'].reduce((x, y) => x + y);
  [['DEAD_flat']].flat();
  [['DEAD_flat_depth']].flat(1);
  ['DEAD_flat_map'].flatMap((x) => [x]);
  ['DEAD_reverse'].reverse();
  ['DEAD_sort'].sort();
  ['DEAD_sort_callback'].sort(() => 0);
  ['DEAD_splice'].splice(0, 1);
  ['DEAD_copy_within'].copyWithin(0, 1);
  ['DEAD_fill'].fill(0);
  return 1;
}
