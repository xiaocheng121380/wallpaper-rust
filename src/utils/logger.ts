import { info, warn, error } from '@tauri-apps/plugin-log'

function safeStringify(v: unknown): string {
  try {
    if (typeof v === 'string') return v
    return JSON.stringify(v)
  } catch {
    try {
      return String(v)
    } catch {
      return '[unserializable]'
    }
  }
}

function format(parts: unknown[]): string {
  return parts.map(safeStringify).join(' ')
}

export const logger = {
  info: (...parts: unknown[]) => {
    const msg = format(parts)
    console.log(msg)
    void info(msg)
  },
  warn: (...parts: unknown[]) => {
    const msg = format(parts)
    console.warn(msg)
    void warn(msg)
  },
  error: (...parts: unknown[]) => {
    const msg = format(parts)
    console.error(msg)
    void error(msg)
  },
}
