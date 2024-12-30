export function a() {
  console.log('a');
}

export function b() {
  return require('./b.js');
}
