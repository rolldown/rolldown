import config from './c.js';

const ccc = await import('./c.js')
export async function getEnv() {
  console.log(111,ccc);
  return config.aaa.bbb[0];
}
