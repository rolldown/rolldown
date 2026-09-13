let x = 'a';
eval("x = 'b'");
export const y = `${x}`;
export const z = x + '';

function assign() {
  eval("w = 'b'");
}
let w = 'a';
assign();
export const v = `${w}`;

// Constant inlining would replace these reads.
export const reads = [y, z, v];
