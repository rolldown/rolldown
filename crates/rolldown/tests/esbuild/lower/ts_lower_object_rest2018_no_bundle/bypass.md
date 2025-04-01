# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
const { ...local_const } = {};
let { ...local_let } = {};
var { ...local_var } = {};
let arrow_fn = ({ ...x2 }) => {
};
let fn_expr = function({ ...x2 } = default_value) {
};
let class_expr = class {
  method(x2, ...[y, { ...z }]) {
  }
};
function fn_stmt({ a = b(), ...x2 }, { c = d(), ...y }) {
}
class class_stmt {
  method({ ...x2 }) {
  }
}
var ns;
((ns2) => {
  ({ ...ns2.x } = {});
})(ns || (ns = {}));
try {
} catch ({ ...catch_clause }) {
}
for (const { ...for_in_const } in { abc }) {
}
for (let { ...for_in_let } in { abc }) {
}
for (var { ...for_in_var } in { abc }) ;
for (const { ...for_of_const } of [{}]) ;
for (let { ...for_of_let } of [{}]) x();
for (var { ...for_of_var } of [{}]) x();
for (const { ...for_const } = {}; x; x = null) {
}
for (let { ...for_let } = {}; x; x = null) {
}
for (var { ...for_var } = {}; x; x = null) {
}
for ({ ...x } in { abc }) {
}
for ({ ...x } of [{}]) {
}
for ({ ...x } = {}; x; x = null) {
}
({ ...assign } = {});
({ obj_method({ ...x2 }) {
} });
({ ...x } = x);
for ({ ...x } = x; 0; ) ;
console.log({ ...x } = x);
console.log({ x, ...xx } = { x });
console.log({ x: { ...xx } } = { x });
```
### rolldown
```js

//#region entry.ts
const { ...local_const } = {};
let { ...local_let } = {};
var { ...local_var } = {};
let ns;
(function(_ns) {
	let { ...x$1 } = {};
	_ns.x = x$1;
})(ns || (ns = {}));
for (const { ...for_in_const } in { abc });
for (let { ...for_in_let } in { abc });
for (var { ...for_in_var } in { abc });
for (const { ...for_of_const } of [{}]);
for (let { ...for_of_let } of [{}]) x();
for (var { ...for_of_var } of [{}]) x();
for (const { ...for_const } = {}; x; x = null);
for (let { ...for_let } = {}; x; x = null);
for (var { ...for_var } = {}; x; x = null);
for ({...x} in { abc });
for ({...x} of [{}]);
for ({...x} = {}; x; x = null);
({...assign} = {});
({...x} = x);
for ({...x} = x; 0;);
console.log({...x} = x);
console.log({x,...xx} = { x });
console.log({x: {...xx}} = { x });
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,45 +1,33 @@
-const {...local_const} = {};
-let {...local_let} = {};
+var {...local_const} = {};
+var {...local_let} = {};
 var {...local_var} = {};
-let arrow_fn = ({...x2}) => {};
-let fn_expr = function ({...x2} = default_value) {};
-let class_expr = class {
-    method(x2, ...[y, {...z}]) {}
-};
-function fn_stmt({a = b(), ...x2}, {c = d(), ...y}) {}
-class class_stmt {
-    method({...x2}) {}
-}
 var ns;
-(ns2 => {
-    ({...ns2.x} = {});
+(function (_ns) {
+    let {...x$1} = {};
+    _ns.x = x$1;
 })(ns || (ns = {}));
-try {} catch ({...catch_clause}) {}
 for (const {...for_in_const} in {
     abc
-}) {}
+}) ;
 for (let {...for_in_let} in {
     abc
-}) {}
+}) ;
 for (var {...for_in_var} in {
     abc
 }) ;
 for (const {...for_of_const} of [{}]) ;
 for (let {...for_of_let} of [{}]) x();
 for (var {...for_of_var} of [{}]) x();
-for (const {...for_const} = {}; x; x = null) {}
-for (let {...for_let} = {}; x; x = null) {}
-for (var {...for_var} = {}; x; x = null) {}
+for (const {...for_const} = {}; x; x = null) ;
+for (let {...for_let} = {}; x; x = null) ;
+for (var {...for_var} = {}; x; x = null) ;
 for ({...x} in {
     abc
-}) {}
-for ({...x} of [{}]) {}
-for ({...x} = {}; x; x = null) {}
+}) ;
+for ({...x} of [{}]) ;
+for ({...x} = {}; x; x = null) ;
 ({...assign} = {});
-({
-    obj_method({...x2}) {}
-});
 ({...x} = x);
 for ({...x} = x; 0; ) ;
 console.log({...x} = x);
 console.log({x, ...xx} = {

```