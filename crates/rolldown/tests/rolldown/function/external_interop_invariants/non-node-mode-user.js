import https from 'node:https';

export function getNonNodeModeDefault() {
  return https;
}
