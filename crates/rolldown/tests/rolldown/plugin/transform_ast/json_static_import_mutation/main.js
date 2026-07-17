import data, { normal } from './data.json';

const before = normal;
data.normal = 9;
const after = normal;

export { after, before, data };
