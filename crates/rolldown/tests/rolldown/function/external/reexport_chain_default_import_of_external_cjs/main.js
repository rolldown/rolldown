import { node_https } from './barrel.js';

export function getAgentCtor() {
  return node_https.Agent;
}
