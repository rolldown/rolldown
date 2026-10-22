function read() {
  return flag ? 'set' : 'unset';
}
let early;
try {
  early = read();
} catch {
  early = 'tdz';
}
let flag = true;
export { early, read };
