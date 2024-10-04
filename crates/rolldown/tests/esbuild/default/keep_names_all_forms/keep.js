// Initializers
function fn() {}
function foo(fn = function() {}) {}
var fn = function() {};
var obj = { "f n": function() {} };
class Foo0 { "f n" = function() {} }
class Foo1 { static "f n" = function() {} }
class Foo2 { accessor "f n" = function() {} }
class Foo3 { static accessor "f n" = function() {} }
class Foo4 { #fn = function() {} }
class Foo5 { static #fn = function() {} }
class Foo6 { accessor #fn = function() {} }
class Foo7 { static accessor #fn = function() {} }

// Assignments
fn = function() {};
fn ||= function() {};
fn &&= function() {};
fn ??= function() {};

// Destructuring
var [fn = function() {}] = [];
var { fn = function() {} } = {};
for (var [fn = function() {}] = []; ; ) ;
for (var { fn = function() {} } = {}; ; ) ;
for (var [fn = function() {}] in obj) ;
for (var { fn = function() {} } in obj) ;
for (var [fn = function() {}] of obj) ;
for (var { fn = function() {} } of obj) ;
function foo([fn = function() {}]) {}
function foo({ fn = function() {} }) {}
[fn = function() {}] = [];
({ fn = function() {} } = {});