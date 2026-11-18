import { eager } from './z.mjs';
console.log(eager(), (await (await import('./z.mjs')).load()).value());
