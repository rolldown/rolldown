# Diff
## /out.js
### esbuild
```js
// entry.js
ok(
  // These should be fully substituted
  1,
  2,
  3,
  // Should just substitute "this.foo"
  2 .baz,
  // This should not be substituted
  1 .bar
);
(() => {
  ok(
    1,
    2,
    3,
    2 .baz,
    1 .bar
  );
})();
(function() {
  doNotSubstitute(
    this,
    this.foo,
    this.foo.bar,
    this.foo.baz,
    this.bar
  );
})();
```
### rolldown
```js

//#region entry.js
ok(
	1,
	2,
	3,
	// Should just substitute "this.foo"
	2 .baz,
	// This should not be substituted
	1 .bar
);
ok(1, 2, 3, 2 .baz, 1 .bar);
(function() {
	doNotSubstitute(this, this.foo, this.foo.bar, this.foo.baz, this.bar);
})();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,5 @@
 ok(1, 2, 3, (2).baz, (1).bar);
-(() => {
-    ok(1, 2, 3, (2).baz, (1).bar);
-})();
+ok(1, 2, 3, (2).baz, (1).bar);
 (function () {
     doNotSubstitute(this, this.foo, this.foo.bar, this.foo.baz, this.bar);
 })();

```