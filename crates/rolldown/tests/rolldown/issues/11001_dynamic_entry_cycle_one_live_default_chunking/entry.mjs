import * as SDK from './sdk.mjs';
console.log(typeof SDK.used);

if ((await SDK.deadA()).a() !== 1) throw new Error('a');
