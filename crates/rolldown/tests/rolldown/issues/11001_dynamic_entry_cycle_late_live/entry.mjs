import * as SDK from './sdk.mjs';
console.log(typeof SDK.used);

if ((await (await SDK.deadA()).a()) !== 2) throw new Error('a');
