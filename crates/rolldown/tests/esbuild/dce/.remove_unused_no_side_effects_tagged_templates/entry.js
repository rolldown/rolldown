// @__NO_SIDE_EFFECTS__
function foo() {}

foo`remove`;
foo`remove${null}`;
foo`remove${123}`;

use(foo`keep`);
foo`remove this part ${keep} and this ${alsoKeep}`;
`remove this part ${keep} and this ${alsoKeep}`;
