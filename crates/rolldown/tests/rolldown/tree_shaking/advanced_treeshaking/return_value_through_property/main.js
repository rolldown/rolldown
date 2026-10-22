const cfg = { debug: false };
function isDebug() {
  return cfg.debug;
}
export function main() {
  if (isDebug()) console.log('DEAD_CFG');
  return 1;
}
