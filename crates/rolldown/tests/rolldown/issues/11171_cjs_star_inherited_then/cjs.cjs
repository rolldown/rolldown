exports.then = 0;
delete exports.then;
Object.setPrototypeOf(exports, { then: (resolve) => resolve('inherited') });
