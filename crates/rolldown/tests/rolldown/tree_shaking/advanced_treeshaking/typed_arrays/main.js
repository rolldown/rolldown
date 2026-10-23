export function main() {
  new Uint8Array(['DEAD_uint8']);
  new Float64Array([1, 'DEAD_float64']);
  new Int32Array(4);
  new Uint16Array();
  return 1;
}
