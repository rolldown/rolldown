import assert from 'node:assert/strict';
import {
  mount,
  read_chained,
  read_live,
  read_cross,
  get_namespace,
  read_namespace,
  read_entry_export,
  read_dynamic,
  load_dynamic,
  read_top,
  read_legacy,
} from './dist/a.js';
import { render_mode, enable_entry_export } from './dist/b.js';

assert.throws(() => mount('app'), { message: 'server' });
assert.equal(render_mode(), 'default');
assert.equal(read_chained(), 'not chained');
assert.equal(read_live(), true);
assert.equal(read_cross(), false);
assert.equal(read_top(), true);
assert.equal(read_legacy(), false);

get_namespace().enable_namespace();
assert.equal(read_namespace(), true);

enable_entry_export();
assert.equal(read_entry_export(), true);

(await load_dynamic()).enable_dynamic();
assert.equal(read_dynamic(), true);
