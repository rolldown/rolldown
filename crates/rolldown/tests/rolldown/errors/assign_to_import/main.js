import { importedA, importedB as b } from "./foo"
import * as ns from './foo'

importedA = 1;

b -= 1;

ns.test += 1;
delete ns.test;

// The name keeps its lone surrogate in the message.
ns['\uD800'] = 1;
