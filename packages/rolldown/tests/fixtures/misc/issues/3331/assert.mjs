import {__default, __rest} from './dist/main'
import assert from 'assert'


assert.deepStrictEqual(Object.keys(__rest).sort(), ["a", "b"]);
assert.equal(__default, "default");
