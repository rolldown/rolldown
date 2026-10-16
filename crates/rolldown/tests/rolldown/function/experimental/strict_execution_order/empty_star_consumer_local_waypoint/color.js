import { DynamicColorIOS, PlatformColor } from "./color-bridge.js"; export function Color(value) { return value; } Color.foreground = () => [PlatformColor, DynamicColorIOS];
