import { foo, _bar } from './print-shorthand-constants'
// The inlined constants must still be present in the output! We don't
// want the printer to use the shorthand syntax here to refer to the
// name of the constant itself because the constant declaration is omitted.
console.log({ foo, _bar })