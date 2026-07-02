import * as data from './data.json' with { type: 'json' };

export const namespace = data;
export const namespaceKeys = Object.keys(data).sort();

console.log(data);
