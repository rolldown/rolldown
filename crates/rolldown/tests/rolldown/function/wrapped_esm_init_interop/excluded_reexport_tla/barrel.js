import { marker } from './carrier.js';
import { used } from './leaf.js';

globalThis.__log.push(`BARREL:${used}:${marker}`);
export const result = used + marker;
