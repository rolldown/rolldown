import { First, inlined } from './first.js';
import { Second } from './second.js';

const Inner = new Second().getInner();
export default [new First().get(), new Inner().get(), inlined.value];
