const isServer = true;
function serverHello() {
  return 'server';
}
function clientHello() {
  return 'DEAD_CLIENT';
}
function pick(s, c) {
  return isServer ? s : c;
}
export const hello = pick(serverHello, clientHello);
