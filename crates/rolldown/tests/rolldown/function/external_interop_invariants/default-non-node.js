import https from 'node:https';

export function getDefaultFromNonNode() {
  return https;
}
