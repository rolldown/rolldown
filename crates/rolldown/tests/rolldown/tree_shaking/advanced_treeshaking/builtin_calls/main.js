// Calls of builtin methods of primitives, regular expression literals, and object literals have
// no effect when the arguments are primitives.
(123).toString();
'DEAD_slice'.slice(1);
'DEAD_trim'.trim();
'DEAD_replace'.replace('x', 'y');
/DEAD_regexp/.test('x');
({ DEAD_has_own: 1 }).hasOwnProperty('x');
Symbol('DEAD_symbol');

// `Object.freeze` of an object that nothing reads.
const frozen = { a: 'DEAD_freeze' };
Object.freeze(frozen);

export const done = 1;
