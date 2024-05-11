//! Only "c0" and "c2" should have "no side effects" (Rollup only respects "const" and only for the first one)
/* #__NO_SIDE_EFFECTS__ */ var v0 = function() {}, v1 = function() {}
/* #__NO_SIDE_EFFECTS__ */ let l0 = function() {}, l1 = function() {}
/* #__NO_SIDE_EFFECTS__ */ const c0 = function() {}, c1 = function() {}
/* #__NO_SIDE_EFFECTS__ */ var v2 = () => {}, v3 = () => {}
/* #__NO_SIDE_EFFECTS__ */ let l2 = () => {}, l3 = () => {}
/* #__NO_SIDE_EFFECTS__ */ const c2 = () => {}, c3 = () => {}