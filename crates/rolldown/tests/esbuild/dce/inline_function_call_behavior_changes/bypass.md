# Reason
1. could be done in minifier
# Diff
## /out/entry.js
### esbuild
```js
function empty() {
}
function id(x) {
  return x;
}
export let shouldBeWrapped = [
  (0, foo.bar)(),
  (0, foo[bar])(),
  (0, foo?.bar)(),
  (0, foo?.[bar])(),
  (0, foo.bar)(),
  (0, foo[bar])(),
  (0, foo?.bar)(),
  (0, foo?.[bar])(),
  (0, eval)(),
  (0, eval)?.(),
  (0, eval)(),
  (0, eval)?.(),
  (0, foo.bar)``,
  (0, foo[bar])``,
  (0, foo?.bar)``,
  (0, foo?.[bar])``,
  (0, foo.bar)``,
  (0, foo[bar])``,
  (0, foo?.bar)``,
  (0, foo?.[bar])``,
  delete (0, foo),
  delete (0, foo.bar),
  delete (0, foo[bar]),
  delete (0, foo?.bar),
  delete (0, foo?.[bar]),
  delete (0, foo),
  delete (0, foo.bar),
  delete (0, foo[bar]),
  delete (0, foo?.bar),
  delete (0, foo?.[bar]),
  delete (0, void 0)
], shouldNotBeWrapped = [
  foo(),
  foo(),
  foo``,
  foo``
], shouldNotBeDoubleWrapped = [
  delete (foo(), bar()),
  delete (foo(), bar())
];
```
### rolldown
```js

//#region entry.js
function empty() {}
function id(x) {
	return x;
}
let shouldBeWrapped = [
	id(foo.bar)(),
	id(foo[bar])(),
	id(foo?.bar)(),
	id(foo?.[bar])(),
	(empty(), foo.bar)(),
	(empty(), foo[bar])(),
	(empty(), foo?.bar)(),
	(empty(), foo?.[bar])(),
	id(eval)(),
	id(eval)?.(),
	(empty(), eval)(),
	(empty(), eval)?.(),
	id(foo.bar)` + "``" + `,
	id(foo[bar])` + "``" + `,
	id(foo?.bar)` + "``" + `,
	id(foo?.[bar])` + "``" + `,
	(empty(), foo.bar)` + "``" + `,
	(empty(), foo[bar])` + "``" + `,
	(empty(), foo?.bar)` + "``" + `,
	(empty(), foo?.[bar])` + "``" + `,
	delete id(foo),
	delete id(foo.bar),
	delete id(foo[bar]),
	delete id(foo?.bar),
	delete id(foo?.[bar]),
	delete (empty(), foo),
	delete (empty(), foo.bar),
	delete (empty(), foo[bar]),
	delete (empty(), foo?.bar),
	delete (empty(), foo?.[bar]),
	delete empty()
];
let shouldNotBeWrapped = [
	id(foo)(),
	(empty(), foo)(),
	id(foo)` + "``" + `,
	(empty(), foo)` + "``" + `
];
let shouldNotBeDoubleWrapped = [delete (empty(), foo(), bar()), delete id((foo(), bar()))];

export { shouldBeWrapped, shouldNotBeDoubleWrapped, shouldNotBeWrapped };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,5 +1,8 @@
 function empty() {}
 function id(x) {
     return x;
 }
-export let shouldBeWrapped = [(0, foo.bar)(), (0, foo[bar])(), (0, foo?.bar)(), (0, foo?.[bar])(), (0, foo.bar)(), (0, foo[bar])(), (0, foo?.bar)(), (0, foo?.[bar])(), (0, eval)(), (0, eval)?.(), (0, eval)(), (0, eval)?.(), (0, foo.bar)``, (0, foo[bar])``, (0, foo?.bar)``, (0, foo?.[bar])``, (0, foo.bar)``, (0, foo[bar])``, (0, foo?.bar)``, (0, foo?.[bar])``, delete (0, foo), delete (0, foo.bar), delete (0, foo[bar]), delete (0, foo?.bar), delete (0, foo?.[bar]), delete (0, foo), delete (0, foo.bar), delete (0, foo[bar]), delete (0, foo?.bar), delete (0, foo?.[bar]), delete (0, void 0)], shouldNotBeWrapped = [foo(), foo(), foo``, foo``], shouldNotBeDoubleWrapped = [delete (foo(), bar()), delete (foo(), bar())];
+var shouldBeWrapped = [id(foo.bar)(), id(foo[bar])(), id(foo?.bar)(), id(foo?.[bar])(), (empty(), foo.bar)(), (empty(), foo[bar])(), (empty(), foo?.bar)(), (empty(), foo?.[bar])(), id(eval)(), id(eval)?.(), (empty(), eval)(), (empty(), eval)?.(), (id(foo.bar))` + "``" + `, (id(foo[bar]))` + "``" + `, (id(foo?.bar))` + "``" + `, (id(foo?.[bar]))` + "``" + `, (empty(), foo.bar)` + "``" + `, (empty(), foo[bar])` + "``" + `, (empty(), foo?.bar)` + "``" + `, (empty(), foo?.[bar])` + "``" + `, delete id(foo), delete id(foo.bar), delete id(foo[bar]), delete id(foo?.bar), delete id(foo?.[bar]), delete (empty(), foo), delete (empty(), foo.bar), delete (empty(), foo[bar]), delete (empty(), foo?.bar), delete (empty(), foo?.[bar]), delete empty()];
+var shouldNotBeWrapped = [id(foo)(), (empty(), foo)(), (id(foo))` + "``" + `, (empty(), foo)` + "``" + `];
+var shouldNotBeDoubleWrapped = [delete (empty(), foo(), bar()), delete id((foo(), bar()))];
+export {shouldBeWrapped, shouldNotBeDoubleWrapped, shouldNotBeWrapped};

```