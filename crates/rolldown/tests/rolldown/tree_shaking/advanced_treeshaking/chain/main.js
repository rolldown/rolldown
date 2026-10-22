const config = { mode: 'prod', DEAD_CFG_MEMBER: 1 };
let mode = config.mode;
function setMode(m) {
  mode = m;
}
function isProd() {
  return mode === 'prod';
}
function devOnly() {
  console.log('DEAD_DEV');
}
export function main() {
  if (!isProd()) {
    devOnly();
  }
  return 1;
}
