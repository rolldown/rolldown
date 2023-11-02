var a;
for (var b; 0;);
// for (var { c, x: [d] } = {}; 0;);
for (var e of []);
for (var { f, x: [g] } of []);
for (var h in {});
// for (var i = 1 in {}); //  the test case is error case, so we don't need to run it   
for (var { j, x: [k] } in {});
function l() {}