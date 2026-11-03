import d, { Agent } from 'node:https';

export function getAgentCtor() {
  return Agent;
}
