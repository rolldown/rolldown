# Diff
## /out.js
### esbuild
```js
// entry.js
var _Foo_instances, g_fn, a_fn, ag_fn, _Foo_static, sg_fn, sa_fn, sag_fn;
var Foo = class {
  constructor() {
    __privateAdd(this, _Foo_instances);
  }
};
_Foo_instances = new WeakSet();
g_fn = function* () {
};
a_fn = async function() {
};
ag_fn = async function* () {
};
_Foo_static = new WeakSet();
sg_fn = function* () {
};
sa_fn = async function() {
};
sag_fn = async function* () {
};
__privateAdd(Foo, _Foo_static);
export {
  Foo
};
```
### rolldown
```js

//#region entry.js
class Foo {
	*#g() {}
	async #a() {}
	async *#ag() {}
	static *#sg() {}
	static async #sa() {}
	static async *#sag() {}
}

//#endregion
export { Foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,16 +1,9 @@
-var _Foo_instances, g_fn, a_fn, ag_fn, _Foo_static, sg_fn, sa_fn, sag_fn;
-var Foo = class {
-    constructor() {
-        __privateAdd(this, _Foo_instances);
-    }
-};
-_Foo_instances = new WeakSet();
-g_fn = function* () {};
-a_fn = async function () {};
-ag_fn = async function* () {};
-_Foo_static = new WeakSet();
-sg_fn = function* () {};
-sa_fn = async function () {};
-sag_fn = async function* () {};
-__privateAdd(Foo, _Foo_static);
+class Foo {
+    *#g() {}
+    async #a() {}
+    async *#ag() {}
+    static *#sg() {}
+    static async #sa() {}
+    static async *#sag() {}
+}
 export {Foo};

```