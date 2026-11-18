import external from 'node:test${(() => { throw new Error("interpolated") })()}`"\'\\\n\r\u2028\u2029';

export const read = () => external.test;
