function getFlag() {
  return false;
}
const flag = getFlag();
export function main() {
  if (flag) console.log('DEAD_RETURN');
  return 1;
}
