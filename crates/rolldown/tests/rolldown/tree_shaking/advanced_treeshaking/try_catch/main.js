globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

const settings = { off: false };

export function main() {
  try {
    if (settings.off) log('DEAD_branch');
    return 1;
    log('DEAD_after_return');
  } catch {
    return 2;
  }
}

try {
} catch {
  log('DEAD_empty_block');
}
