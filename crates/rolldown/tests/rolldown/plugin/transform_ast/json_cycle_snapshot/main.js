import data, { normal } from './data.json';

const beforeMutation = normal;
data.normal = 9;
const afterMutation = normal;

export { afterMutation, beforeMutation, data };
