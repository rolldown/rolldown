import d from 'node:https';

export function getServerCtor() {
  return d.Server;
}
