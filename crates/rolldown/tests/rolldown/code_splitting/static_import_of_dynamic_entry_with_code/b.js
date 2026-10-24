import { Bar, foo } from './lib/index.js';

export const b = () => new Bar().value() + foo.value;
