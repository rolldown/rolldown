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

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res) => function() {
	return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};
var __copyProps = (to, from, except, desc) => {
	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
		key = keys[i];
		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
			get: ((k) => from[k]).bind(null, key),
			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
		});
	}
	return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

//#region foo.js
var foo_exports = {};
__export(foo_exports, { foo: () => foo$1 });
function foo$1() {
	return "foo";
}
var init_foo = __esm({ "foo.js"() {} });

//#region bar.js
var bar_exports = {};
__export(bar_exports, { bar: () => bar$1 });
function bar$1() {
	return "bar";
}
var init_bar = __esm({ "bar.js"() {} });

//#region entry.js
const { foo } = (init_foo(), __toCommonJS(
	// This should not be hoisted
	foo_exports
));
assert.equal(foo(), "foo");
assert.equal(bar(), "bar");
const { bar } = (init_bar(), __toCommonJS(bar_exports));

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,23 +1,49 @@
+var __defProp = Object.defineProperty;
+var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __hasOwnProp = Object.prototype.hasOwnProperty;
+var __esm = (fn, res) => function () {
+    return (fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res);
+};
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var __copyProps = (to, from, except, desc) => {
+    if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
+        key = keys[i];
+        if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
+            get: (k => from[k]).bind(null, key),
+            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
+        });
+    }
+    return to;
+};
+var __toCommonJS = mod => __copyProps(__defProp({}, "__esModule", {
+    value: true
+}), mod);
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