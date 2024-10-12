# Diff
## /out.js
### esbuild
```js
// entry.js
var A = class {
  #a;
  f() {
    this.#a ?? (this.#a = 1);
  }
};
export {
  A
};
```
### rolldown
```js

//#region entry.js
class A {
	#a;
	f() {
		this.#a ??= 1;
	}
}

//#endregion
export { A };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,7 @@
-var A = class {
+class A {
     #a;
     f() {
-        this.#a ?? (this.#a = 1);
+        this.#a ??= 1;
     }
-};
+}
 export {A};

```