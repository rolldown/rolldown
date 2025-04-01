# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
var _e;
export class A {
}
export class B extends A {
  constructor(c) {
    var _a;
    super();
    __privateAdd(this, _e);
    __privateSet(this, _e, (_a = c.d) != null ? _a : "test");
  }
  f() {
    return __privateGet(this, _e);
  }
}
_e = new WeakMap();
```
### rolldown
```js

//#region entry.ts
var A = class {};
var B = class extends A {
	#e;
	constructor(c) {
		super();
		this.#e = c.d ?? "test";
	}
	f() {
		return this.#e;
	}
};
//#endregion

export { A, B };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,12 @@
-var _e;
-export class A {}
-export class B extends A {
+var A = class {};
+var B = class extends A {
+    #e;
     constructor(c) {
-        var _a;
         super();
-        __privateAdd(this, _e);
-        __privateSet(this, _e, (_a = c.d) != null ? _a : "test");
+        this.#e = c.d ?? "test";
     }
     f() {
-        return __privateGet(this, _e);
+        return this.#e;
     }
-}
-_e = new WeakMap();
+};
+export {A, B};

```