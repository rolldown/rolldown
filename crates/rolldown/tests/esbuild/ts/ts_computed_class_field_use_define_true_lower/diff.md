# Diff
## /out.js
### esbuild
```js
var _a, _b, _c, _d;
_d = q, _c = r, _b = x, _a = y;
class Foo {
  constructor() {
    __publicField(this, _d);
    __publicField(this, _c, s);
    __publicField(this, _b);
    __publicField(this, _a, z);
  }
}
__decorateClass([
  dec
], Foo.prototype, _b, 2);
__decorateClass([
  dec
], Foo.prototype, _a, 2);
new Foo();
```
### rolldown
```js

//#region entry.ts
class Foo {
	[q];
	[r] = s;
	@dec [x];
	@dec [y] = z;
}
new Foo();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,11 @@
-var _a, _b, _c, _d;
-(_d = q, _c = r, _b = x, _a = y);
+
+//#region entry.ts
 class Foo {
-    constructor() {
-        __publicField(this, _d);
-        __publicField(this, _c, s);
-        __publicField(this, _b);
-        __publicField(this, _a, z);
-    }
+	[q];
+	[r] = s;
+	@dec [x];
+	@dec [y] = z;
 }
-__decorateClass([dec], Foo.prototype, _b, 2);
-__decorateClass([dec], Foo.prototype, _a, 2);
 new Foo();
+
+//#endregion
\ No newline at end of file

```