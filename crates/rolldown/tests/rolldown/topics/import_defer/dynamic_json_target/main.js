import defer * as staticNs from './data.json';

const tag = (value) => Object.prototype.toString.call(value);
export async function run() {
  const same = await import.defer('./data.json');
  const other = await import.defer('./other.json');
  const otherAgain = await import.defer('./other.json');
  return {
    sameChunk: { tag: tag(same), identity: same === staticNs, a: same.a },
    otherChunk: { tag: tag(other), identity: other === otherAgain, b: other.b },
  };
}
