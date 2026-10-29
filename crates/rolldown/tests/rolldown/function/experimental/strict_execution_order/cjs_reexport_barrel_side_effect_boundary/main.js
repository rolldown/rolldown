import { cn } from 'pure-boundary-barrel';
import { nestedCn } from './ancestor.js';

globalThis.__events.push(`main:${cn()}:${nestedCn()}`);

export const loadRoute = () => import('./route.js');
