globalThis.__events.push('dep');

export default function dep() {
  return 'D';
}

export const named = 'N';
