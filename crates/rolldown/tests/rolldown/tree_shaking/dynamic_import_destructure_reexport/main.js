const ns = await import('./lib.js');
export const { used } = ns; // re-exported, not read locally -> must survive
console.log(ns.other); // second ref prevents alias inlining
