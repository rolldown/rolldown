export const early = [];
export const p = import.defer('./d.js');
export const pj = import.defer('./data.json');
// These callbacks run while `pause.js` waits.
p.then((ns) => early.push(['d', ns.keys]));
pj.then((ns) => early.push(['data', ns.a]));
