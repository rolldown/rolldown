# Reason
1. oxc transform strip type decl but did not remove related `ExportDecl`, this woulcause rolldown assume it export a global variable, which has side effects.
# Diff
## /out.js
### esbuild
```js
// keep/declare-class.ts
var declare_class_default = foo;
var bar = 123;

// keep/declare-let.ts
var declare_let_default = foo;
var bar2 = 123;

// keep/interface-merged.ts
var foo2 = class _foo {
  static {
    this.x = new _foo();
  }
};
var interface_merged_default = foo2;
var bar3 = 123;

// keep/interface-nested.ts
if (true) {
}
var interface_nested_default = foo;
var bar4 = 123;

// keep/type-nested.ts
if (true) {
}
var type_nested_default = foo;
var bar5 = 123;

// keep/value-namespace.ts
var foo3;
((foo5) => {
  foo5.num = 0;
})(foo3 || (foo3 = {}));
var value_namespace_default = foo3;
var bar6 = 123;

// keep/value-namespace-merged.ts
var foo4;
((foo5) => {
  foo5.num = 0;
})(foo4 || (foo4 = {}));
var value_namespace_merged_default = foo4;
var bar7 = 123;

// remove/interface.ts
var bar8 = 123;

// remove/interface-exported.ts
var bar9 = 123;

// remove/type.ts
var bar10 = 123;

// remove/type-exported.ts
var bar11 = 123;

// remove/type-only-namespace.ts
var bar12 = 123;

// remove/type-only-namespace-exported.ts
var bar13 = 123;

// entry.ts
var entry_default = [
  declare_class_default,
  bar,
  declare_let_default,
  bar2,
  interface_merged_default,
  bar3,
  interface_nested_default,
  bar4,
  type_nested_default,
  bar5,
  value_namespace_default,
  bar6,
  value_namespace_merged_default,
  bar7,
  bar8,
  bar9,
  bar10,
  bar11,
  bar12,
  bar13
];
export {
  entry_default as default
};
```
### rolldown
```js

//#region keep/declare-class.ts
var declare_class_default = foo;
let dc = 123;

//#endregion
//#region keep/declare-let.ts
var declare_let_default = foo;
let dl = 123;

//#endregion
//#region keep/interface-merged.ts
var foo$2 = class foo$2 {
	static x = new foo$2();
};
var interface_merged_default = foo$2;
let im = 123;

//#endregion
//#region keep/interface-nested.ts
var interface_nested_default = foo;
let _in = 123;

//#endregion
//#region keep/type-nested.ts
var type_nested_default = foo;
let tn = 123;

//#endregion
//#region keep/value-namespace.ts
let foo$1;
(function(_foo) {
	let num = _foo.num = 0;
})(foo$1 || (foo$1 = {}));
var value_namespace_default = foo$1;
let vn = 123;

//#endregion
//#region keep/value-namespace-merged.ts
(function(_foo2) {
	let num = _foo2.num = 0;
})(foo || (foo = {}));
var value_namespace_merged_default = foo;
let vnm = 123;

//#endregion
//#region remove/interface.ts
var interface_default = foo;
let i = 123;

//#endregion
//#region remove/interface-exported.ts
var interface_exported_default = foo;
let ie = 123;

//#endregion
//#region remove/type.ts
var type_default = foo;
let t = 123;

//#endregion
//#region remove/type-exported.ts
var type_exported_default = foo;
let te = 123;

//#endregion
//#region remove/type-only-namespace.ts
var type_only_namespace_default = foo;
let ton = 123;

//#endregion
//#region remove/type-only-namespace-exported.ts
var type_only_namespace_exported_default = foo;
let tone = 123;

//#endregion
//#region entry.ts
var entry_default = [
	declare_class_default,
	dc,
	declare_let_default,
	dl,
	interface_merged_default,
	im,
	interface_nested_default,
	_in,
	type_nested_default,
	tn,
	value_namespace_default,
	vn,
	value_namespace_merged_default,
	vnm,
	i,
	ie,
	t,
	te,
	ton,
	tone
];

//#endregion
export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,37 +1,38 @@
 var declare_class_default = foo;
-var bar = 123;
+var dc = 123;
 var declare_let_default = foo;
-var bar2 = 123;
-var foo2 = class _foo {
-    static {
-        this.x = new _foo();
-    }
+var dl = 123;
+var foo$2 = class foo$2 {
+    static x = new foo$2();
 };
-var interface_merged_default = foo2;
-var bar3 = 123;
-if (true) {}
+var interface_merged_default = foo$2;
+var im = 123;
 var interface_nested_default = foo;
-var bar4 = 123;
-if (true) {}
+var _in = 123;
 var type_nested_default = foo;
-var bar5 = 123;
-var foo3;
-(foo5 => {
-    foo5.num = 0;
-})(foo3 || (foo3 = {}));
-var value_namespace_default = foo3;
-var bar6 = 123;
-var foo4;
-(foo5 => {
-    foo5.num = 0;
-})(foo4 || (foo4 = {}));
-var value_namespace_merged_default = foo4;
-var bar7 = 123;
-var bar8 = 123;
-var bar9 = 123;
-var bar10 = 123;
-var bar11 = 123;
-var bar12 = 123;
-var bar13 = 123;
-var entry_default = [declare_class_default, bar, declare_let_default, bar2, interface_merged_default, bar3, interface_nested_default, bar4, type_nested_default, bar5, value_namespace_default, bar6, value_namespace_merged_default, bar7, bar8, bar9, bar10, bar11, bar12, bar13];
+var tn = 123;
+var foo$1;
+(function (_foo) {
+    let num = _foo.num = 0;
+})(foo$1 || (foo$1 = {}));
+var value_namespace_default = foo$1;
+var vn = 123;
+(function (_foo2) {
+    let num = _foo2.num = 0;
+})(foo || (foo = {}));
+var value_namespace_merged_default = foo;
+var vnm = 123;
+var interface_default = foo;
+var i = 123;
+var interface_exported_default = foo;
+var ie = 123;
+var type_default = foo;
+var t = 123;
+var type_exported_default = foo;
+var te = 123;
+var type_only_namespace_default = foo;
+var ton = 123;
+var type_only_namespace_exported_default = foo;
+var tone = 123;
+var entry_default = [declare_class_default, dc, declare_let_default, dl, interface_merged_default, im, interface_nested_default, _in, type_nested_default, tn, value_namespace_default, vn, value_namespace_merged_default, vnm, i, ie, t, te, ton, tone];
 export {entry_default as default};

```