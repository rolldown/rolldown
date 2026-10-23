export function main() {
  (123).toString(10, 'DEAD_to_string');
  (1.5).toFixed(1, 'DEAD_to_fixed');
  (1.5).toPrecision(2, 'DEAD_to_precision');
  (1.5).toExponential(1, 'DEAD_to_exponential');
  return 1;
}
