import fooModule, { barModule } from 'external-module';
import * as bazModule from 'external-module';
import fooCjs, { barCjs } from 'external-cjs';
import * as bazCjs from 'external-cjs';
import fooDefault, { barDefault } from 'external-cjs-with-default';
import * as bazDefault from 'external-cjs-with-default';
import update from 'update';

assert.deepStrictEqual(fooModule, { barModule: 'bar', default: 'foo' }, 'module');
assert.strictEqual(barModule, 'bar', 'module');
assert.deepStrictEqual(
	bazModule,
	{ __proto__: null, barModule: 'bar', default: { barModule: 'bar', default: 'foo' } },
	'module'
);
assert.deepStrictEqual(fooCjs, { barCjs: 'bar' }, 'cjs');
assert.strictEqual(barCjs, 'bar', 'cjs');
assert.deepStrictEqual(
	bazCjs,
	{ __proto__: null, barCjs: 'bar', default: { barCjs: 'bar' } },
	'cjs'
);
assert.deepStrictEqual(fooDefault, { barDefault: 'bar', default: 'foo' }, 'default');
assert.strictEqual(barDefault, 'bar', 'default');
assert.deepStrictEqual(
	bazDefault,
	{ __proto__: null, barDefault: 'bar', default: { barDefault: 'bar', default: 'foo' } },
	'default'
);

update();

assert.deepStrictEqual(fooModule, { barModule: 'bar2', default: 'foo2' }, 'module');
assert.strictEqual(barModule, 'bar2', 'module');
assert.deepStrictEqual(
	bazModule,
	{ __proto__: null, barModule: 'bar2', default: { barModule: 'bar2', default: 'foo2' } },
	'module'
);
assert.deepStrictEqual(fooCjs, { barCjs: 'bar2' }, 'cjs');
assert.strictEqual(barCjs, 'bar2', 'cjs');
assert.deepStrictEqual(
	bazCjs,
	{ __proto__: null, barCjs: 'bar2', default: { barCjs: 'bar2' } },
	'cjs'
);
assert.deepStrictEqual(fooDefault, { barDefault: 'bar2', default: 'foo2' }, 'default');
assert.strictEqual(barDefault, 'bar2', 'default');
assert.deepStrictEqual(
	bazDefault,
	{ __proto__: null, barDefault: 'bar2', default: { barDefault: 'bar2', default: 'foo2' } },
	'default'
);
