// The finalizer prints `request` as a member of the node-mode namespace. The parameter
// `node_https$1` must not capture that namespace, so the namespace must not take this name.
import https, { request } from 'node:https';

export function read(node_https$1) {
  return [typeof request, node_https$1, typeof https.request];
}
