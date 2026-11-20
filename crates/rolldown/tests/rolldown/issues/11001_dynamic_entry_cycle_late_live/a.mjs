import { used, deadB } from './sdk.mjs';
export function a() {
  return deadB().then((m) => m.b() + used());
}
