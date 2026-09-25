import { unused } from '@unused';
import { nested } from '@nested';
import { effectful } from '@effectful';

export const dead = () => [unused(), nested(), effectful()];
export default 'kept';
