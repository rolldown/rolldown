# Reason
1. different deconflict naming convention
# Diff
## /out.js
### esbuild
```js
// foo.js
var foo_exports = {};
__export(foo_exports, {
  foo: () => foo
});
function foo() {
  return "foo";
}
var init_foo = __esm({
  "foo.js"() {
  }
});

// bar.js
var bar_exports = {};
__export(bar_exports, {
  bar: () => bar
});
function bar() {
  return "bar";
}
var init_bar = __esm({
  "bar.js"() {
  }
});

// entry.js
var { foo: foo2 } = (init_foo(), __toCommonJS(foo_exports));
console.log(foo2(), bar2());
var { bar: bar2 } = (init_bar(), __toCommonJS(bar_exports));
```
### rolldown
```js
import assert from "node:assert";

// HIDDEN [rolldown:runtime]
//#region foo.js
var foo_exports = {};
__export(foo_exports, { foo: () => foo$1 });
function foo$1() {
	return "foo";
}
var init_foo = __esm({ "foo.js"() {} });

//#endregion
//#region bar.js
var bar_exports = {};
__export(bar_exports, { bar: () => bar$1 });
function bar$1() {
	return "bar";
}
var init_bar = __esm({ "bar.js"() {} });

//#endregion
//#region entry.js
const { foo } = (init_foo(), __toCommonJS(foo_exports));
assert.equal(foo(), "foo");
assert.equal(bar(), "bar");
const { bar } = (init_bar(), __toCommonJS(bar_exports));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,23 +1,23 @@
 var foo_exports = {};
 __export(foo_exports, {
-    foo: () => foo
+    foo: () => foo$1
 });
-function foo() {
+function foo$1() {
     return "foo";
 }
 var init_foo = __esm({
     "foo.js"() {}
 });
 var bar_exports = {};
 __export(bar_exports, {
-    bar: () => bar
+    bar: () => bar$1
 });
-function bar() {
+function bar$1() {
     return "bar";
 }
 var init_bar = __esm({
     "bar.js"() {}
 });
-var {foo: foo2} = (init_foo(), __toCommonJS(foo_exports));
-console.log(foo2(), bar2());
-var {bar: bar2} = (init_bar(), __toCommonJS(bar_exports));
+var {foo} = (init_foo(), __toCommonJS(foo_exports));
+var {bar} = (init_bar(), __toCommonJS(bar_exports));
+console.log(foo(), bar());

```