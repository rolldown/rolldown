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
//#region keep/interface-merged.ts
var foo$3 = class foo$3 {
	static {
		this.x = new foo$3();
	}
};

//#endregion
//#region keep/interface-nested.ts
var interface_nested_default = foo;

//#endregion
//#region keep/type-nested.ts
var type_nested_default = foo;

//#endregion
//#region keep/value-namespace.ts
let foo$2;
(function(_foo) {
	_foo.num = 0;
})(foo$2 || (foo$2 = {}));
var value_namespace_default = foo$2;

//#endregion
//#region keep/value-namespace-merged.ts
let foo$1;
(function(_foo) {
	_foo.num = 0;
})(foo$1 || (foo$1 = {}));
var value_namespace_merged_default = foo$1;

//#endregion
//#region entry.ts
var entry_default = [
	dc_def,
	123,
	dl_def,
	123,
	foo$3,
	123,
	interface_nested_default,
	123,
	type_nested_default,
	123,
	value_namespace_default,
	123,
	value_namespace_merged_default,
	123,
	123,
	123,
	123,
	123,
	123,
	123
];

//#endregion
export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,37 +1,19 @@
-var declare_class_default = foo;
-var bar = 123;
-var declare_let_default = foo;
-var bar2 = 123;
-var foo2 = class _foo {
+var foo$3 = class foo$3 {
     static {
-        this.x = new _foo();
+        this.x = new foo$3();
     }
 };
-var interface_merged_default = foo2;
-var bar3 = 123;
-if (true) {}
 var interface_nested_default = foo;
-var bar4 = 123;
-if (true) {}
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
+var foo$2;
+(function (_foo) {
+    _foo.num = 0;
+})(foo$2 || (foo$2 = {}));
+var value_namespace_default = foo$2;
+var foo$1;
+(function (_foo) {
+    _foo.num = 0;
+})(foo$1 || (foo$1 = {}));
+var value_namespace_merged_default = foo$1;
+var entry_default = [dc_def, 123, dl_def, 123, foo$3, 123, interface_nested_default, 123, type_nested_default, 123, value_namespace_default, 123, value_namespace_merged_default, 123, 123, 123, 123, 123, 123, 123];
 export {entry_default as default};

```