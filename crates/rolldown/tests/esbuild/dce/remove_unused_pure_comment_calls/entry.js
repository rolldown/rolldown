function bar() {}
let bare = foo(bar);

let at_yes = /* @__PURE__ */ foo(bar);
let at_no = /* @__PURE__ */ foo(bar());
let new_at_yes = /* @__PURE__ */ new foo(bar);
let new_at_no = /* @__PURE__ */ new foo(bar());

let nospace_at_yes = /*@__PURE__*/ foo(bar);
let nospace_at_no = /*@__PURE__*/ foo(bar());
let nospace_new_at_yes = /*@__PURE__*/ new foo(bar);
let nospace_new_at_no = /*@__PURE__*/ new foo(bar());

let num_yes = /* #__PURE__ */ foo(bar);
let num_no = /* #__PURE__ */ foo(bar());
let new_num_yes = /* #__PURE__ */ new foo(bar);
let new_num_no = /* #__PURE__ */ new foo(bar());

let nospace_num_yes = /*#__PURE__*/ foo(bar);
let nospace_num_no = /*#__PURE__*/ foo(bar());
let nospace_new_num_yes = /*#__PURE__*/ new foo(bar);
let nospace_new_num_no = /*#__PURE__*/ new foo(bar());

let dot_yes = /* @__PURE__ */ foo(sideEffect()).dot(bar);
let dot_no = /* @__PURE__ */ foo(sideEffect()).dot(bar());
let new_dot_yes = /* @__PURE__ */ new foo(sideEffect()).dot(bar);
let new_dot_no = /* @__PURE__ */ new foo(sideEffect()).dot(bar());

let nested_yes = [1, /* @__PURE__ */ foo(bar), 2];
let nested_no = [1, /* @__PURE__ */ foo(bar()), 2];
let new_nested_yes = [1, /* @__PURE__ */ new foo(bar), 2];
let new_nested_no = [1, /* @__PURE__ */ new foo(bar()), 2];

let single_at_yes = // @__PURE__
	foo(bar);
let single_at_no = // @__PURE__
	foo(bar());
let new_single_at_yes = // @__PURE__
	new foo(bar);
let new_single_at_no = // @__PURE__
	new foo(bar());

let single_num_yes = // #__PURE__
	foo(bar);
let single_num_no = // #__PURE__
	foo(bar());
let new_single_num_yes = // #__PURE__
	new foo(bar);
let new_single_num_no = // #__PURE__
	new foo(bar());

let bad_no = /* __PURE__ */ foo(bar);
let new_bad_no = /* __PURE__ */ new foo(bar);

let parens_no = (/* @__PURE__ */ foo)(bar);
let new_parens_no = new (/* @__PURE__ */ foo)(bar);

let exp_no = /* @__PURE__ */ foo() ** foo();
let new_exp_no = /* @__PURE__ */ new foo() ** foo();
