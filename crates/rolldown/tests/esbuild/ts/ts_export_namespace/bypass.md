# Reason
1. the extra `let foo` could be stripped by minifier
# Diff
## /out.js
### esbuild
```js
// b.ts
var Foo = class {
};
((Foo2) => {
  Foo2.foo = 1;
})(Foo || (Foo = {}));
((Foo2) => {
  Foo2.bar = 2;
})(Foo || (Foo = {}));

// a.ts
console.log(new Foo());
```
### rolldown
```js
//#region b.ts
var Foo = class {};
(function(_Foo) {
	_Foo.foo = 1;
})(Foo || (Foo = {}));
(function(_Foo2) {
	_Foo2.bar = 2;
})(Foo || (Foo = {}));

//#endregion
//#region a.ts
console.log(new Foo());

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	a.js
@@ -1,8 +1,8 @@
 var Foo = class {};
-(Foo2 => {
-    Foo2.foo = 1;
+(function (_Foo) {
+    _Foo.foo = 1;
 })(Foo || (Foo = {}));
-(Foo2 => {
-    Foo2.bar = 2;
+(function (_Foo2) {
+    _Foo2.bar = 2;
 })(Foo || (Foo = {}));
 console.log(new Foo());

```