const { ...local_const } = {};
let { ...local_let } = {};
var { ...local_var } = {};
let arrow_fn = ({ ...x }) => { };
let fn_expr = function ({ ...x } = default_value) {};
let class_expr = class { method(x, ...[y, { ...z }]) {} };

function fn_stmt({ a = b(), ...x }, { c = d(), ...y }) {}
class class_stmt { method({ ...x }) {} }
namespace ns { export let { ...x } = {} }
try { } catch ({ ...catch_clause }) {}

for (const { ...for_in_const } in { abc }) {}
for (let { ...for_in_let } in { abc }) {}
for (var { ...for_in_var } in { abc }) ;
for (const { ...for_of_const } of [{}]) ;
for (let { ...for_of_let } of [{}]) x()
for (var { ...for_of_var } of [{}]) x()
for (const { ...for_const } = {}; x; x = null) {}
for (let { ...for_let } = {}; x; x = null) {}
for (var { ...for_var } = {}; x; x = null) {}
for ({ ...x } in { abc }) {}
for ({ ...x } of [{}]) {}
for ({ ...x } = {}; x; x = null) {}

({ ...assign } = {});
({ obj_method({ ...x }) {} });

// Check for used return values
({ ...x } = x);
for ({ ...x } = x; 0; ) ;
console.log({ ...x } = x);
console.log({ x, ...xx } = { x });
console.log({ x: { ...xx } } = { x });