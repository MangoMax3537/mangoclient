import type { LogLevel } from './types';

/** Classifies one log line for colouring (Minecraft/log4j style, plus stack traces). */
export function logLevel(line: string): LogLevel {
  if (/\/(ERROR|FATAL)\]|\[(ERROR|FATAL)\]|^\s*(Caused by:|Exception in thread)|^\s+at\s+[\w$.]+\(|^---- Minecraft Crash Report|Exception:/.test(line)) return 'error';
  if (/\/WARN\]|\[WARN\]/.test(line)) return 'warn';
  return 'info';
}
