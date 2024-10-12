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
let bar$12 = 123;

//#endregion
//#region keep/declare-let.ts
var declare_let_default = foo;
let bar$11 = 123;

//#endregion
//#region keep/interface-merged.ts
class foo$3 {
	static x = new foo$3();
}
var interface_merged_default = foo$3;
let bar$10 = 123;

//#endregion
//#region keep/interface-nested.ts
var interface_nested_default = foo;
let bar$9 = 123;

//#endregion
//#region keep/type-nested.ts
var type_nested_default = foo;
let bar$8 = 123;

//#endregion
//#region keep/value-namespace.ts
let foo$2;
(function(_foo) {
	let num = _foo.num = 0;
})(foo$2 || (foo$2 = {}));
var value_namespace_default = foo$2;
let bar$7 = 123;

//#endregion
//#region keep/value-namespace-merged.ts
let foo$1;
(function(_foo2) {
	let num = _foo2.num = 0;
})(foo$1 || (foo$1 = {}));
var value_namespace_merged_default = foo$1;
let bar$6 = 123;

//#endregion
//#region remove/interface.ts
var interface_default = foo;
let bar$5 = 123;

//#endregion
//#region remove/interface-exported.ts
var interface_exported_default = foo;
let bar$4 = 123;

//#endregion
//#region remove/type.ts
var type_default = foo;
let bar$3 = 123;

//#endregion
//#region remove/type-exported.ts
var type_exported_default = foo;
let bar$2 = 123;

//#endregion
//#region remove/type-only-namespace.ts
var type_only_namespace_default = foo;
let bar$1 = 123;

//#endregion
//#region remove/type-only-namespace-exported.ts
var type_only_namespace_exported_default = foo;
let bar = 123;

//#endregion
//#region entry.ts
var entry_default = [
	declare_class_default,
	bar$12,
	declare_let_default,
	bar$11,
	interface_merged_default,
	bar$10,
	interface_nested_default,
	bar$9,
	type_nested_default,
	bar$8,
	value_namespace_default,
	bar$7,
	value_namespace_merged_default,
	bar$6,
	bar$5,
	bar$4,
	bar$3,
	bar$2,
	bar$1,
	bar
];

//#endregion
export { entry_default as default };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,37 +1,39 @@
 var declare_class_default = foo;
-var bar = 123;
+var bar$12 = 123;
 var declare_let_default = foo;
-var bar2 = 123;
-var foo2 = class _foo {
-    static {
-        this.x = new _foo();
-    }
-};
-var interface_merged_default = foo2;
-var bar3 = 123;
-if (true) {}
+var bar$11 = 123;
+class foo$3 {
+    static x = new foo$3();
+}
+var interface_merged_default = foo$3;
+var bar$10 = 123;
 var interface_nested_default = foo;
-var bar4 = 123;
-if (true) {}
+var bar$9 = 123;
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
+var bar$8 = 123;
+var foo$2;
+(function (_foo) {
+    let num = _foo.num = 0;
+})(foo$2 || (foo$2 = {}));
+var value_namespace_default = foo$2;
+var bar$7 = 123;
+var foo$1;
+(function (_foo2) {
+    let num = _foo2.num = 0;
+})(foo$1 || (foo$1 = {}));
+var value_namespace_merged_default = foo$1;
+var bar$6 = 123;
+var interface_default = foo;
+var bar$5 = 123;
+var interface_exported_default = foo;
+var bar$4 = 123;
+var type_default = foo;
+var bar$3 = 123;
+var type_exported_default = foo;
+var bar$2 = 123;
+var type_only_namespace_default = foo;
+var bar$1 = 123;
+var type_only_namespace_exported_default = foo;
+var bar = 123;
+var entry_default = [declare_class_default, bar$12, declare_let_default, bar$11, interface_merged_default, bar$10, interface_nested_default, bar$9, type_nested_default, bar$8, value_namespace_default, bar$7, value_namespace_merged_default, bar$6, bar$5, bar$4, bar$3, bar$2, bar$1, bar];
 export {entry_default as default};

```