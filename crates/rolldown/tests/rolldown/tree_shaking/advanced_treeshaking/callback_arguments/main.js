export function main() {
  ['DEAD_map_index'].map((x, i) => i + 1);
  ['DEAD_unary'].map((x) => +x);
  ['DEAD_bitwise'].map((x) => x | 0);
  ['DEAD_template'].map((x) => `${x}`);
  [1, 2, 'DEAD_sort_numeric'].sort((a, b) => a - b);
  ['DEAD_reduce_sum'].reduce((acc, x) => acc + x.length, 0);
  ['DEAD_array_parameter'].every((x, i, all) => all.length > i);
  return 1;
}
