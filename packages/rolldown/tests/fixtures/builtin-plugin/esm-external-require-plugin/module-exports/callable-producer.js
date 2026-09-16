import callable from './callable.cjs';

export default 'not the CommonJS export';
export { callable as 'module.exports' };
