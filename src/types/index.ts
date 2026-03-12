export interface Wallpaper {
  id: string
  title: string
  localPath: string
  thumbnailPath?: string
  resolution?: string
  fileSize?: number
  importTime: string
}

export interface Settings {
  schemaVersion: number
  language: string
  themeMode: 'light' | 'dark' | 'system'
  primaryColor?: string
}

export interface CommandResult<T> {
  ok: boolean
  data?: T
  error?: CommandError
}

export interface CommandError {
  code: string
  message: string
}
