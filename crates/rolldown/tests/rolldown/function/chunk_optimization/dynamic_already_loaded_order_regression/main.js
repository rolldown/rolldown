import './base.js';
import './consumer.js';
import './read.js';
import { service } from './barrel.js';

export const value = service;
export const loadBase = () => import('./base.js');
export const loadRead = () => import('./read.js');
export const loadSetup = () => import('./setup.js');
export const loadForward = () => import('./forward.js');
export const loadConsumer = () => import('./consumer.js');
export const loadSplash = () => import('./splash.js');
export const loadBarrel = () => import('./barrel.js').then((m) => m.service);
