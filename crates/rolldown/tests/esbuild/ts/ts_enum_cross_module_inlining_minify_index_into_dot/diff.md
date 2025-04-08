# Reason
1. not support const enum inline
# Diff
## /out.js
### esbuild
```js
// entry.ts
inlined = [
  obj.abc,
  obj.xyz,
  obj?.abc,
  obj?.xyz,
  obj?.prop.abc,
  obj?.prop.xyz
];
notInlined = [
  obj["a b c" /* foo2 */],
  obj["x y z" /* bar2 */],
  obj?.["a b c" /* foo2 */],
  obj?.["x y z" /* bar2 */],
  obj?.prop["a b c" /* foo2 */],
  obj?.prop["x y z" /* bar2 */]
];
```
### rolldown
```js

//#region lib.ts
let Bar = /* @__PURE__ */ function(Bar) {
	Bar["bar1"] = "xyz";
	Bar["bar2"] = "x y z";
	return Bar;
}({});

//#endregion
//#region entry.ts
var Foo = /* @__PURE__ */ function(Foo) {
	Foo["foo1"] = "abc";
	Foo["foo2"] = "a b c";
	return Foo;
}(Foo || {});
inlined = [
	obj[Foo.foo1],
	obj[Bar.bar1],
	obj?.[Foo.foo1],
	obj?.[Bar.bar1],
	obj?.prop[Foo.foo1],
	obj?.prop[Bar.bar1]
];
notInlined = [
	obj[Foo.foo2],
	obj[Bar.bar2],
	obj?.[Foo.foo2],
	obj?.[Bar.bar2],
	obj?.prop[Foo.foo2],
	obj?.prop[Bar.bar2]
];

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,12 @@
-inlined = [obj.abc, obj.xyz, obj?.abc, obj?.xyz, obj?.prop.abc, obj?.prop.xyz];
-notInlined = [obj["a b c"], obj["x y z"], obj?.["a b c"], obj?.["x y z"], obj?.prop["a b c"], obj?.prop["x y z"]];
+var Bar = (function (Bar) {
+    Bar["bar1"] = "xyz";
+    Bar["bar2"] = "x y z";
+    return Bar;
+})({});
+var Foo = (function (Foo) {
+    Foo["foo1"] = "abc";
+    Foo["foo2"] = "a b c";
+    return Foo;
+})(Foo || ({}));
+inlined = [obj[Foo.foo1], obj[Bar.bar1], obj?.[Foo.foo1], obj?.[Bar.bar1], obj?.prop[Foo.foo1], obj?.prop[Bar.bar1]];
+notInlined = [obj[Foo.foo2], obj[Bar.bar2], obj?.[Foo.foo2], obj?.[Bar.bar2], obj?.prop[Foo.foo2], obj?.prop[Bar.bar2]];

```