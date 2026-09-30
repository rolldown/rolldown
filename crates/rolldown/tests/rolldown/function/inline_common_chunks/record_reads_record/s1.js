import { base, tag } from './s3.js';
globalThis.events.push('S1');
export const one = base.n + 1;
export const tagged = tag`t${one}`;
export class C {
  constructor() {
    this.k = 'c';
  }
}
