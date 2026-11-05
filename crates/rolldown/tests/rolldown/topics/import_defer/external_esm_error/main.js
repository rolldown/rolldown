import defer * as ns from 'ext';

export const read = () => ns.value;
export const load = () => import.defer('ext2');
