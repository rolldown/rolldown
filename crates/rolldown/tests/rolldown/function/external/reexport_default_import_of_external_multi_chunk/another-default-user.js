import { node_https } from './reexport.js';

export function getServerCtor() {
  return node_https.Server;
}
