export function loadLightA() {
  return import('./light-consumer-a.js');
}

export function loadLightB() {
  return import('./light-consumer-b.js');
}

export function loadHeavy() {
  return import('./heavy-consumer.js');
}
