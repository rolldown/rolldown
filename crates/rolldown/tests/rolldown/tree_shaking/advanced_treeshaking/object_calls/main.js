export function main() {
  Object.freeze(['DEAD_freeze_array']);
  Object.freeze(function DEAD_freeze_function() {});
  Object.freeze(() => 'DEAD_freeze_arrow');
  Object.freeze(/DEAD_freeze_regexp/);
  Object.values({ x: 'DEAD_values' });
  Object.entries({ x: 'DEAD_entries' });
  Object.values(['DEAD_values_array']);
  Object.values('DEAD_values_string');
  return 1;
}
