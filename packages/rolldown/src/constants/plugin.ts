import { Plugin } from '../plugin'

/**
 * Names of all properties in a `Plugin` object. Includes `name` and `api`.
 */
export type PluginProps = keyof Plugin

/**
 * Names of all hooks in a `Plugin` object. Does not include `name` and `api`, since they are not hooks.
 */
export type PluginHookNames = Exclude<PluginProps, 'name' | 'api'>
