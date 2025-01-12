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

//#region ../../../../../../node_modules/.pnpm/@babel+runtime@7.26.0/node_modules/@babel/runtime/helpers/esm/checkPrivateRedeclaration.js
function _checkPrivateRedeclaration(e, t) {
	if (t.has(e)) throw new TypeError("Cannot initialize the same private elements twice on an object");
}

//#endregion
//#region ../../../../../../node_modules/.pnpm/@babel+runtime@7.26.0/node_modules/@babel/runtime/helpers/esm/classPrivateFieldInitSpec.js
function _classPrivateFieldInitSpec(e, t, a) {
	_checkPrivateRedeclaration(e, t), t.set(e, a);
}

//#endregion
//#region ../../../../../../node_modules/.pnpm/@babel+runtime@7.26.0/node_modules/@babel/runtime/helpers/esm/assertClassBrand.js
function _assertClassBrand(e, t, n) {
	if ("function" == typeof e ? e === t : e.has(t)) return arguments.length < 3 ? t : n;
	throw new TypeError("Private element is not present on this object");
}

//#endregion
//#region ../../../../../../node_modules/.pnpm/@babel+runtime@7.26.0/node_modules/@babel/runtime/helpers/esm/classPrivateFieldGet2.js
function _classPrivateFieldGet2(s, a) {
	return s.get(_assertClassBrand(s, a));
}

//#endregion
//#region ../../../../../../node_modules/.pnpm/@babel+runtime@7.26.0/node_modules/@babel/runtime/helpers/esm/classPrivateFieldSet2.js
function _classPrivateFieldSet2(s, a, r) {
	return s.set(_assertClassBrand(s, a), r), r;
}

//#endregion
//#region entry.js
var _a = new WeakMap();
var A = class {
	constructor() {
		_classPrivateFieldInitSpec(this, _a, void 0);
	}
	f() {
		var _classPrivateFieldGet2$1;
		(_classPrivateFieldGet2$1 = _classPrivateFieldGet2(_a, this)) !== null && _classPrivateFieldGet2$1 !== void 0 ? _classPrivateFieldGet2$1 : _classPrivateFieldSet2(_a, this, 1);
	}
};

//#endregion
export { A };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,27 @@
+function _checkPrivateRedeclaration(e, t) {
+    if (t.has(e)) throw new TypeError("Cannot initialize the same private elements twice on an object");
+}
+function _classPrivateFieldInitSpec(e, t, a) {
+    (_checkPrivateRedeclaration(e, t), t.set(e, a));
+}
+function _assertClassBrand(e, t, n) {
+    if ("function" == typeof e ? e === t : e.has(t)) return arguments.length < 3 ? t : n;
+    throw new TypeError("Private element is not present on this object");
+}
+function _classPrivateFieldGet2(s, a) {
+    return s.get(_assertClassBrand(s, a));
+}
+function _classPrivateFieldSet2(s, a, r) {
+    return (s.set(_assertClassBrand(s, a), r), r);
+}
+var _a = new WeakMap();
 var A = class {
-    #a;
+    constructor() {
+        _classPrivateFieldInitSpec(this, _a, void 0);
+    }
     f() {
-        this.#a ?? (this.#a = 1);
+        var _classPrivateFieldGet2$1;
+        (_classPrivateFieldGet2$1 = _classPrivateFieldGet2(_a, this)) !== null && _classPrivateFieldGet2$1 !== void 0 ? _classPrivateFieldGet2$1 : _classPrivateFieldSet2(_a, this, 1);
     }
 };
 export {A};

```