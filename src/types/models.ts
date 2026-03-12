export type WallpaperType = 'image' | 'video' | 'web'

export interface WallpaperItem {
  id: string
  type: WallpaperType
  title: string
  favorite: boolean
  tags: string[]
}
