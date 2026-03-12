<script setup lang="ts">
defineOptions({ name: 'LibraryView' })

import {
  inject,
  nextTick,
  onActivated,
  onDeactivated,
  onMounted,
  onUnmounted,
  ref,
  type Ref,
} from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  NButton,
  NCard,
  NEmpty,
  NGrid,
  NGridItem,
  NImage,
  NSkeleton,
  NSpace,
  NScrollbar,
  NText,
  NTooltip,
} from 'naive-ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { logger } from '../utils/logger'
import { api } from '../api'
import { revokeObjectUrl, videoBytesToObjectUrl } from '../utils/media'

const { t } = useI18n()
const showAlert = inject<(message: string) => void>('showAlert')

interface Wallpaper {
  id: string
  title: string
  thumbnail: string
  isVideo?: boolean
  importTime: string
  localPath: string
  resolution?: string
  fileSize?: number
  metadata?: any
}

const wallpapers = ref<Wallpaper[]>([])
const selectedWallpaper = inject<Ref<Wallpaper | null>>('selectedWallpaper')
const isLoading = ref(false)
const imageCache = new Map<string, string>()
const currentWallpaperId = ref<string | null>(null)
const isGeneratingPreviews = ref(false)
const previewGenerationDone = ref(0)
const previewGenerationTotal = ref(0)
let previewGenerationToken = 0

const isLinux = ref(false)

const linuxMediaBaseUrl = ref('')

const autoPlayVideoThumbs = ref(true)
const isWindowVisible = ref(true)

let lastApplyKey: string | null = null
let lastApplyAt = 0

const videoPlaceholderDataUrl =
  'data:image/svg+xml;utf8,' +
  encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360">
      <defs>
        <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="#1b1b1f"/>
          <stop offset="1" stop-color="#111115"/>
        </linearGradient>
      </defs>
      <rect width="640" height="360" fill="url(#g)"/>
      <circle cx="320" cy="180" r="52" fill="rgba(255,255,255,0.10)"/>
      <path d="M304 152 L304 208 L352 180 Z" fill="rgba(255,255,255,0.75)"/>
    </svg>`
  )

function isVideoPath(p: string) {
  const ext = (p.split('.').pop() || '').toLowerCase()
  return ['mp4', 'webm', 'mkv', 'avi', 'mov', 'wmv', 'flv', 'm4v'].includes(ext)
}

const onLibraryChanged = () => {
  loadWallpapers()
}

const onWallpaperApplied = (e: Event) => {
  const customEvent = e as CustomEvent
  const id = customEvent.detail?.id
  logger.info('[库] 收到壁纸应用事件，id:', id)
  if (id) {
    currentWallpaperId.value = id
    logger.info('[库] 已更新当前壁纸 id:', currentWallpaperId.value)
  }
}

function pauseAllVideos() {
  const videos = document.querySelectorAll('.wallpaper-card video')
  videos.forEach((video) => {
    if (video instanceof HTMLVideoElement) {
      video.pause()
    }
  })
}

function resumeAllVideos() {
  const videos = document.querySelectorAll('.wallpaper-card video')
  videos.forEach((video) => {
    if (video instanceof HTMLVideoElement && autoPlayVideoThumbs.value) {
      video.play().catch(() => { })
    }
  })
}

function onPreviewVideoEnded(e: Event) {
  const v = e.target as HTMLVideoElement | null
  if (!v) return
  try {
    v.currentTime = 0
  } catch {
    // ignore
  }

  // In some WebKit/GStreamer setups (notably in VMs), looping may require a full reload
  // if seeking is not supported by the underlying pipeline.
  if (!Number.isFinite(v.currentTime) || v.currentTime > 0.01) {
    const src = v.currentSrc || v.src
    if (src) {
      const u = new URL(src, window.location.href)
      u.searchParams.set('_loop', String(Date.now()))
      v.src = u.toString()
    }
  }

  v.play().catch(() => { })
}

function onPreviewVideoTimeUpdate(e: Event) {
  const v = e.target as HTMLVideoElement | null
  if (!v) return
  const d = v.duration
  if (!Number.isFinite(d) || d <= 0) return

  // Manual loop: some WebKit/GStreamer setups in VMs do not reliably honor `loop`
  // or do not fire `ended` when seeking is limited.
  if (v.currentTime >= d - 0.12) {
    try {
      v.currentTime = 0
    } catch {
      // ignore
    }
    v.play().catch(() => { })
  }
}

function onPreviewVideoPaused(e: Event) {
  const v = e.target as HTMLVideoElement | null
  if (!v) return
  if (!autoPlayVideoThumbs.value) return
  if (document.hidden) return
  if (v.ended) return
  // Some pipelines pause unexpectedly; attempt to resume.
  v.play().catch(() => { })
}

onMounted(async () => {
  const platform = await api.systemGetPlatform().catch(() => ({ ok: false } as any))
  isLinux.value = !!(platform?.ok && platform.data === 'linux')

  if (isLinux.value) {
    const baseRes = await api.systemGetMediaBaseUrl().catch(() => ({ ok: false } as any))
    if (baseRes?.ok && typeof baseRes.data === 'string') {
      linuxMediaBaseUrl.value = baseRes.data
    }
  }

  await loadWallpapers()

  // Listen for window visibility changes
  const appWindow = getCurrentWindow()
  const unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
    isWindowVisible.value = focused
    if (!focused) {
      pauseAllVideos()
    } else {
      resumeAllVideos()
    }
  })

  onUnmounted(() => {
    unlistenFocus()
  })

  window.addEventListener('library:changed', onLibraryChanged as any)
  window.addEventListener('wallpaper:applied', onWallpaperApplied as any)
})

onUnmounted(() => {
  // Cleanup object urls
  for (const url of imageCache.values()) {
    revokeObjectUrl(url)
  }
  window.removeEventListener('library:changed', onLibraryChanged as any)
  window.removeEventListener('wallpaper:applied', onWallpaperApplied as any)
})

onActivated(() => {
  if (!autoPlayVideoThumbs.value) return
  nextTick(() => {
    const videos = document.querySelectorAll<HTMLVideoElement>('.wallpaper-card .card-image video')
    videos.forEach(v => {
      try {
        v.play()?.catch(() => {
          // ignore
        })
      } catch {
        // ignore
      }
    })
  })
})

onDeactivated(() => {
  const videos = document.querySelectorAll<HTMLVideoElement>('.wallpaper-card .card-image video')
  videos.forEach(v => {
    try {
      v.pause()
    } catch {
      // ignore
    }
  })
})

async function loadWallpapers() {
  isLoading.value = true
  const generationToken = ++previewGenerationToken
  isGeneratingPreviews.value = false
  previewGenerationDone.value = 0
  previewGenerationTotal.value = 0
  try {
    // Load settings to get current wallpaper ID
    const settingsResult = await api.settingsGet()
    if (settingsResult.ok && settingsResult.data?.current_wallpaper_id) {
      currentWallpaperId.value = settingsResult.data.current_wallpaper_id
      logger.info('[库] 当前壁纸 id:', currentWallpaperId.value)
    } else {
      logger.info('[库] 未找到当前壁纸 id')
    }

    const result = await api.libraryList()
    if (result.ok && result.data) {
      const mapped = result.data.map(w => ({
        id: w.id,
        title: w.title,
        thumbnail: w.thumbnail_path
          ? isVideoPath(w.thumbnail_path)
            ? (isLinux.value ? videoPlaceholderDataUrl : convertFileSrc(w.thumbnail_path))
            : convertFileSrc(w.thumbnail_path)
          : isVideoPath(w.local_path)
            ? videoPlaceholderDataUrl
            : convertFileSrc(w.local_path),
        isVideo: isVideoPath(w.local_path),
        importTime: w.import_time,
        localPath: w.local_path,
        resolution: w.resolution,
        fileSize: w.file_size,
        metadata: (w as any).metadata,
      }))
      wallpapers.value = mapped

      // Linux: load video preview bytes and use blob URL
      if (isLinux.value) {
        const videoThumbs = result.data.filter(
          w => !!w.thumbnail_path && isVideoPath(w.thumbnail_path)
        )
        if (videoThumbs.length > 0) {
          await Promise.all(
            videoThumbs.map(w => loadVideoPreviewAsync(w.id, w.thumbnail_path!))
          )
        }
      }

      // Auto-select current wallpaper
      if (currentWallpaperId.value && selectedWallpaper) {
        const current = mapped.find(w => w.id === currentWallpaperId.value)
        if (current) {
          selectedWallpaper.value = current as any
        }
      }

      const pendingThumbnails = result.data.filter(w => !w.thumbnail_path)
      if (pendingThumbnails.length > 0) {
        previewGenerationTotal.value = pendingThumbnails.length
        isGeneratingPreviews.value = true
        void processPreviewQueue(
          pendingThumbnails.map(w => ({ id: w.id, path: w.local_path })),
          generationToken
        )
      }
    }
  } catch (e) {
    logger.error('加载壁纸列表失败:', e)
  } finally {
    isLoading.value = false
  }
}

async function loadVideoPreviewAsync(id: string, filePath: string) {
  // Linux: avoid asset:// range quirks and avoid streaming bytes; use custom scheme instead.
  if (isLinux.value) {
    const base = linuxMediaBaseUrl.value
    const url = base
      ? `${base}/file?path=${encodeURIComponent(filePath)}`
      : `wallcraft://localhost/${encodeURIComponent(filePath)}`
    const prev = imageCache.get(id)
    if (prev) {
      revokeObjectUrl(prev)
    }
    imageCache.set(id, url)
    updateWallpaperMedia(id, url, true)
    return
  }

  try {
    const res = await api.getVideoBytes(filePath)
    if (res.ok && res.data) {
      const url = videoBytesToObjectUrl(filePath, res.data)
      const prev = imageCache.get(id)
      if (prev) {
        revokeObjectUrl(prev)
      }
      imageCache.set(id, url)
      updateWallpaperMedia(id, url, true)
      return
    }
  } catch (e) {
    logger.warn('加载视频预览字节失败:', { id, filePath, error: e })
  }
}

async function loadImageAsync(id: string, path: string) {
  if (imageCache.has(id)) {
    updateWallpaperThumbnail(id, imageCache.get(id)!)
    return
  }

  try {
    const result = await api.getThumbnailPath(id, path)
    if (result.ok && result.data) {
      if (result.data.startsWith('video:')) {
        const filePath = result.data.slice('video:'.length)
        if (isLinux.value) {
          await loadVideoPreviewAsync(id, filePath)
          return
        }
        const url = convertFileSrc(filePath)
        imageCache.set(id, url)
        updateWallpaperMedia(id, url, true)
        return
      }

      if (result.data === 'video') {
        imageCache.set(id, videoPlaceholderDataUrl)
        updateWallpaperMedia(id, videoPlaceholderDataUrl, true)
        return
      }

      const url = convertFileSrc(result.data)
      imageCache.set(id, url)
      updateWallpaperMedia(id, url, false)
      return
    }

    // 兜底：失败时
    // - 图片：用原图
    // - 视频：不要用原视频做缩略图（会触发解码导致 GPU 飙升），用占位图
    const isVideo = isVideoPath(path)
    const fallbackUrl = isVideo ? videoPlaceholderDataUrl : convertFileSrc(path)
    imageCache.set(id, fallbackUrl)
    updateWallpaperMedia(id, fallbackUrl, isVideo)
    if (!result.ok) {
      logger.error('获取缩略图路径失败:', { id, path, error: result.error })
    } else {
      logger.warn('获取缩略图路径返回空数据:', { id, path })
    }
  } catch (e) {
    // 兜底：异常时
    // - 图片：用原图
    // - 视频：不要用原视频做缩略图（会触发解码导致 GPU 飙升），用占位图
    const isVideo = isVideoPath(path)
    const fallbackUrl = isVideo ? videoPlaceholderDataUrl : convertFileSrc(path)
    imageCache.set(id, fallbackUrl)
    updateWallpaperMedia(id, fallbackUrl, isVideo)
    logger.error('加载缩略图失败:', { id, path, error: e })
  }
}

async function processPreviewQueue(
  items: Array<{ id: string; path: string }>,
  token: number,
) {
  const concurrency = 2
  let nextIndex = 0

  const worker = async () => {
    while (nextIndex < items.length && token === previewGenerationToken) {
      const currentIndex = nextIndex
      nextIndex += 1
      const item = items[currentIndex]
      if (!item) {
        continue
      }

      await loadImageAsync(item.id, item.path)

      if (token !== previewGenerationToken) {
        return
      }

      previewGenerationDone.value += 1
      await new Promise(resolve => window.setTimeout(resolve, 0))
    }
  }

  await Promise.all(
    Array.from({ length: Math.min(concurrency, items.length) }, () => worker())
  )

  if (token === previewGenerationToken) {
    isGeneratingPreviews.value = false
  }
}

function previewGenerationPercent() {
  if (previewGenerationTotal.value <= 0) {
    return 0
  }

  return Math.min(
    100,
    Math.round((previewGenerationDone.value / previewGenerationTotal.value) * 100)
  )
}

function updateWallpaperMedia(id: string, url: string, isVideo: boolean) {
  const idx = wallpapers.value.findIndex(w => w.id === id)
  if (idx !== -1 && wallpapers.value[idx]) {
    wallpapers.value[idx]!.thumbnail = url
    wallpapers.value[idx]!.isVideo = isVideo
  }
}

function updateWallpaperThumbnail(id: string, dataUrl: string) {
  const idx = wallpapers.value.findIndex(w => w.id === id)
  if (idx !== -1 && wallpapers.value[idx]) {
    wallpapers.value[idx]!.thumbnail = dataUrl
  }
}

async function handleImport(type: string) {
  try {
    let paths: string[] | null = null

    if (type === 'files') {
      paths = await api.openFileDialog()
    } else if (type === 'folder') {
      const folder = await api.openFolderDialog()
      if (folder) {
        paths = [folder]
      }
    }

    if (!paths || paths.length === 0) return

    isLoading.value = true
    const result = await api.libraryImport(paths)
    if (result.ok) {
      await loadWallpapers()
    } else {
      logger.error('导入壁纸失败:', result.error)
    }
  } catch (e) {
    logger.error('导入壁纸异常:', e)
  } finally {
    isLoading.value = false
  }
}

async function handleApply(wallpaper: Wallpaper) {
  const now = Date.now()
  const key = `${wallpaper.id}|${wallpaper.isVideo ? 'video' : 'image'}`
  if (lastApplyKey === key && now - lastApplyAt < 400) {
    return
  }
  lastApplyKey = key
  lastApplyAt = now

  const path = wallpaper.localPath
  const type = wallpaper.isVideo ? 'video' : 'image'
  const res = await api.wallpaperApply(wallpaper.id, path, type)
  if (!res.ok) {
    logger.error('应用壁纸失败:', res.error)
    if (res.error?.code === 'VIDEO_WALLPAPER_NOT_SUPPORTED') {
      showAlert?.(t('videoWallpaperNotSupported'))
    } else {
      showAlert?.(res.error?.message ?? t('applyFailed'))
    }
  } else {
    // Update current wallpaper ID on success
    currentWallpaperId.value = wallpaper.id
    logger.info('[库] 应用壁纸成功，已更新当前壁纸 id:', currentWallpaperId.value)
  }
}

function selectWallpaper(wallpaper: Wallpaper) {
  if (selectedWallpaper) {
    selectedWallpaper.value = wallpaper as any
  }
}

function handleContextMenu(e: MouseEvent, wallpaper: Wallpaper) {
  e.preventDefault()
  selectWallpaper(wallpaper)
}
</script>

<template>
  <div class="library-container">
    <!-- 顶部工具栏 -->
    <header class="sub-header">
      <NSpace justify="space-between" align="center" :wrap="false" style="width: 100%">
        <NSpace align="center" :wrap="false">
          <NText class="page-title" strong>{{ t('library') }}</NText>
          <NText v-if="wallpapers.length > 0" depth="3" class="wallpaper-count">
            {{ wallpapers.length }} {{ t('wallpaperCount') }}
          </NText>
          <div v-if="isGeneratingPreviews" class="header-generating-chip">
            <span class="header-generating-dot" />
            <NText depth="3" class="header-generating-text">
              生成预览中 {{ previewGenerationDone }}/{{ previewGenerationTotal }}
            </NText>
          </div>
        </NSpace>
        <NSpace align="center" :wrap="false">
          <NButton type="primary" size="small" @click="handleImport('files')">{{ t('importAction') }}</NButton>
        </NSpace>
      </NSpace>
    </header>

    <!-- 空状态 -->
    <NScrollbar class="library-content" v-if="!isLoading && wallpapers.length === 0">
      <div class="empty-state">
        <NEmpty :description="t('noWallpapersHint')">
          <template #default>
            <NText strong>{{ t('noWallpapers') }}</NText>
          </template>
          <template #extra>
            <NSpace>
              <NButton type="primary" size="large" @click="handleImport('files')">
                {{ t('importFiles') }}
              </NButton>
              <NButton size="large" @click="handleImport('folder')">{{ t('importFolder') }}</NButton>
            </NSpace>
          </template>
        </NEmpty>
      </div>
    </NScrollbar>

    <!-- 缩略图网格区 -->
    <NScrollbar class="library-content" v-if="isLoading || wallpapers.length > 0">
      <div v-if="isLoading" class="wallpaper-grid">
        <NGrid cols="1 s:2 m:3 l:4 xl:5" responsive="screen" :x-gap="16" :y-gap="16">
          <NGridItem v-for="n in 10" :key="n">
            <NCard size="small" class="wallpaper-card" :bordered="false">
              <template #cover>
                <div class="card-image">
                  <NSkeleton height="100%" width="100%" />
                </div>
              </template>
              <template #footer>
                <NSkeleton text :repeat="1" />
              </template>
            </NCard>
          </NGridItem>
        </NGrid>
      </div>

      <div v-else class="wallpaper-grid">
        <div v-if="isGeneratingPreviews" class="preview-generation-overlay">
          <div class="preview-generation-panel">
            <div class="preview-generation-spinner">
              <span />
              <span />
              <span />
            </div>
            <div class="preview-generation-copy">
              <div class="preview-generation-title">正在生成预览</div>
              <div class="preview-generation-subtitle">
                {{ previewGenerationDone }} / {{ previewGenerationTotal }}，你现在可以继续操作页面
              </div>
            </div>
            <div class="preview-generation-progress">
              <div
                class="preview-generation-progress-bar"
                :style="{ width: `${previewGenerationPercent()}%` }"
              />
            </div>
          </div>
        </div>
        <NGrid cols="1 s:2 m:3 l:4 xl:5" responsive="screen" :x-gap="16" :y-gap="16">
          <NGridItem v-for="wallpaper in wallpapers" :key="wallpaper.id">
            <NCard size="small" hoverable class="wallpaper-card" :bordered="false" :class="{
              selected: selectedWallpaper?.id === wallpaper.id,
              current: currentWallpaperId === wallpaper.id
            }" :data-current-label="t('current')" @click="selectWallpaper(wallpaper)"
              @dblclick="handleApply(wallpaper)" @contextmenu="handleContextMenu($event, wallpaper)">
              <template #cover>
                <div class="card-image">
                  <video v-if="wallpaper.isVideo && autoPlayVideoThumbs" :src="wallpaper.thumbnail" muted loop
                    playsinline autoplay preload="metadata" @ended="onPreviewVideoEnded"
                    @timeupdate="onPreviewVideoTimeUpdate" @pause="onPreviewVideoPaused" />
                  <NImage v-else :src="wallpaper.thumbnail" :alt="wallpaper.title" object-fit="cover" preview-disabled
                    style="width: 100%; height: 100%" />
                </div>
              </template>

              <template #footer>
                <NSpace justify="space-between" align="center" :wrap="false">
                  <NTooltip trigger="hover">
                    <template #trigger>
                      <NText class="card-title">{{ wallpaper.title || t('untitled') }}</NText>
                    </template>
                    {{ wallpaper.title || t('untitled') }}
                  </NTooltip>
                </NSpace>
              </template>
            </NCard>
          </NGridItem>
        </NGrid>
      </div>
    </NScrollbar>
  </div>
</template>

<style scoped>
.library-container {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.sub-header {
  padding: 12px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.page-title {
  font-size: 16px;
}

.wallpaper-count {
  font-size: 12px;
}

.header-generating-chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.06);
}

.header-generating-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #18a058;
  box-shadow: 0 0 0 0 rgba(24, 160, 88, 0.55);
  animation: preview-pulse 1.6s infinite;
}

.header-generating-text {
  font-size: 12px;
}

.library-content {
  flex: 1;
  height: 100%;
  padding: 16px;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  min-height: 400px;
}

.wallpaper-grid {
  width: 100%;
  padding: 4px;
  position: relative;
}

.preview-generation-overlay {
  position: sticky;
  top: 0;
  z-index: 20;
  display: flex;
  justify-content: flex-end;
  pointer-events: none;
}

.preview-generation-panel {
  width: min(360px, calc(100% - 24px));
  margin: 0 8px 16px auto;
  padding: 12px 14px;
  border-radius: 14px;
  background: rgba(20, 20, 24, 0.88);
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
  backdrop-filter: blur(10px);
}

.preview-generation-spinner {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}

.preview-generation-spinner span {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #18a058;
  animation: preview-bounce 0.9s infinite ease-in-out;
}

.preview-generation-spinner span:nth-child(2) {
  animation-delay: 0.12s;
}

.preview-generation-spinner span:nth-child(3) {
  animation-delay: 0.24s;
}

.preview-generation-copy {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 10px;
}

.preview-generation-title {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.96);
}

.preview-generation-subtitle {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.68);
}

.preview-generation-progress {
  width: 100%;
  height: 6px;
  border-radius: 999px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.08);
}

.preview-generation-progress-bar {
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, #18a058 0%, #36cfc9 100%);
  transition: width 180ms ease;
}

.wallpaper-card {
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  transition: box-shadow 160ms ease;
}

.wallpaper-card:hover {
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.22);
}

.wallpaper-card.selected {
  border-color: var(--primary-color, #18a058);
  box-shadow: 0 0 0 2px var(--primary-color, #18a058);
}

.wallpaper-card.current {
  border-color: #18a058;
  box-shadow: 0 0 0 3px #18a058;
  position: relative;
}

.wallpaper-card.current::after {
  content: attr(data-current-label);
  position: absolute;
  top: 8px;
  right: 8px;
  background: #18a058;
  color: white;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  font-weight: 500;
  z-index: 10;
  pointer-events: none;
}

.card-image {
  position: relative;
  aspect-ratio: 16/9;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.2);
}

.card-image :deep(.n-image),
.card-image :deep(.n-image > img),
.card-image :deep(.n-image img) {
  width: 100%;
  height: 100%;
}

.card-image :deep(.n-image img) {
  object-fit: cover;
  transition: transform 180ms ease;
  will-change: transform;
}

.card-image video {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: transform 180ms ease;
  will-change: transform;
}

.wallpaper-card:hover .card-image :deep(.n-image img),
.wallpaper-card:hover .card-image video {
  transform: scale(1.4);
  transform-origin: center;
}

.card-title {
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

@keyframes preview-pulse {
  0% {
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(24, 160, 88, 0.55);
  }
  70% {
    transform: scale(1.05);
    box-shadow: 0 0 0 8px rgba(24, 160, 88, 0);
  }
  100% {
    transform: scale(1);
    box-shadow: 0 0 0 0 rgba(24, 160, 88, 0);
  }
}

@keyframes preview-bounce {
  0%,
  80%,
  100% {
    transform: translateY(0);
    opacity: 0.45;
  }
  40% {
    transform: translateY(-4px);
    opacity: 1;
  }
}
</style>
