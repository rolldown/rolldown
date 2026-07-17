import * as data from './data.json';

const before = data.normal;
data.default.normal = 9;
const after = data.normal;

export { after, before, data };
