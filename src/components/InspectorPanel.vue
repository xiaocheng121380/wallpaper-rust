<script setup lang="ts">
import { computed, inject, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { NButton, NDivider, NEmpty, NImage, NInput, NScrollbar, NSpace, NText } from 'naive-ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { logger } from '../utils/logger'
import { api } from '../api'
import { revokeObjectUrl } from '../utils/media'

const props = defineProps<{ selectedWallpaper: any | null }>()
const emit = defineEmits<{ (e: 'update:selectedWallpaper', value: any | null): void }>()

const { t } = useI18n()
const showAlert = inject<(message: string) => void>('showAlert')

const inspectorVideoRef = ref<HTMLVideoElement | null>(null)
const isWindowVisible = ref(true)

let lastApplyAt = 0

const inspectorFilePath = computed(() => {
    const w: any = props.selectedWallpaper
    return w?.localPath ?? w?.path ?? ''
})

const inspectorVideoSrc = ref('')

const linuxMediaBaseUrl = ref('')

let linuxRetryTimer: any = null

async function refreshInspectorVideoSrc() {
    const w: any = props.selectedWallpaper
    if (!w || !w.isVideo) {
        if (linuxRetryTimer) {
            clearTimeout(linuxRetryTimer)
            linuxRetryTimer = null
        }
        revokeObjectUrl(inspectorVideoSrc.value)
        inspectorVideoSrc.value = ''
        return
    }

    const p = inspectorFilePath.value
    if (!p) {
        revokeObjectUrl(inspectorVideoSrc.value)
        inspectorVideoSrc.value = ''
        return
    }

    const platform = await api.systemGetPlatform().catch(() => ({ ok: false } as any))
    if (platform?.ok && platform.data === 'linux') {
        try {
            if (!linuxMediaBaseUrl.value) {
                const baseRes = await api.systemGetMediaBaseUrl().catch(() => ({ ok: false } as any))
                if (baseRes?.ok && typeof baseRes.data === 'string') {
                    linuxMediaBaseUrl.value = baseRes.data
                }
            }

            const id = w?.id ?? ''
            const playPathRes = id ? await api.getDetailPlayPath(id, p) : ({ ok: false } as any)
            const playPath = playPathRes?.ok ? playPathRes.data : ''
            if (!playPath) {
                if (!linuxRetryTimer) {
                    linuxRetryTimer = setTimeout(() => {
                        linuxRetryTimer = null
                        refreshInspectorVideoSrc()
                    }, 1500)
                }
                revokeObjectUrl(inspectorVideoSrc.value)
                inspectorVideoSrc.value = ''
                return
            }

            const base = linuxMediaBaseUrl.value
            const url = base ? `${base}/file?path=${encodeURIComponent(playPath)}` : `wallcraft://localhost/${encodeURIComponent(playPath)}`
            revokeObjectUrl(inspectorVideoSrc.value)
            inspectorVideoSrc.value = url
            return
        } catch (e) {
            logger.warn('加载详情视频字节失败:', { path: p, error: e })
        }

        revokeObjectUrl(inspectorVideoSrc.value)
        inspectorVideoSrc.value = ''
        return
    }

    revokeObjectUrl(inspectorVideoSrc.value)
    inspectorVideoSrc.value = convertFileSrc(p)
}

const inspectorPosterSrc = computed(() => {
    const w: any = props.selectedWallpaper
    const src: string = w?.thumbnail ?? ''
    if (!src) return ''
    if (src.startsWith('data:image/')) return src
    const lower = src.toLowerCase()
    if (
        lower.endsWith('.jpg') ||
        lower.endsWith('.jpeg') ||
        lower.endsWith('.png') ||
        lower.endsWith('.webp') ||
        lower.endsWith('.gif')
    ) {
        return src
    }
    return ''
})

watch(inspectorFilePath, () => {
    refreshInspectorVideoSrc()
}, { immediate: true })

function formatBytes(bytes?: number) {
    if (bytes === undefined || bytes === null) return ''
    if (bytes < 1024) return `${bytes} B`
    const kb = bytes / 1024
    if (kb < 1024) return `${kb.toFixed(1)} KB`
    const mb = kb / 1024
    if (mb < 1024) return `${mb.toFixed(1)} MB`
    const gb = mb / 1024
    return `${gb.toFixed(1)} GB`
}

const inspectorFileSize = computed(() => {
    const w: any = props.selectedWallpaper
    const raw = w?.fileSize ?? w?.size
    if (typeof raw === 'number') return formatBytes(raw)
    if (typeof raw === 'string') return raw
    return ''
})

const inspectorImportTime = computed(() => {
    const w: any = props.selectedWallpaper
    return w?.importTime ?? w?.import_time ?? ''
})

const inspectorResolution = computed(() => {
    const w: any = props.selectedWallpaper
    const r = w?.resolution
    if (typeof r === 'string' && r.trim()) return r
    const meta = w?.metadata
    if (meta && meta.width && meta.height) return `${meta.width}×${meta.height}`
    return ''
})

const inspectorFormat = computed(() => {
    const w: any = props.selectedWallpaper
    const p = inspectorFilePath.value
    const ext = (p.split('.').pop() || '').trim().toLowerCase()
    if (ext) return ext

    const meta = w?.metadata
    if (meta?.format_name) {
        const raw = String(meta.format_name)
        const first = raw.split(',')[0]?.trim()
        return first || raw
    }
    return ''
})

function formatDuration(sec?: number) {
    if (sec === undefined || sec === null) return ''
    if (!Number.isFinite(sec)) return ''
    const total = Math.max(0, Math.floor(sec))
    const h = Math.floor(total / 3600)
    const m = Math.floor((total % 3600) / 60)
    const s = total % 60
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
    return `${m}:${String(s).padStart(2, '0')}`
}

function formatBitRate(bps?: number) {
    if (bps === undefined || bps === null) return ''
    if (!Number.isFinite(bps)) return ''
    if (bps < 1000) return `${Math.round(bps)} bps`
    const kbps = bps / 1000
    if (kbps < 1000) return `${kbps.toFixed(0)} kbps`
    const mbps = kbps / 1000
    return `${mbps.toFixed(2)} Mbps`
}

const inspectorDuration = computed(() => {
    const w: any = props.selectedWallpaper
    const sec = w?.metadata?.duration_sec
    if (typeof sec === 'number') return formatDuration(sec)
    return ''
})

const inspectorVideoCodec = computed(() => {
    const w: any = props.selectedWallpaper
    const c = w?.metadata?.video_codec
    if (!c) return ''
    const pix = w?.metadata?.pix_fmt
    return pix ? `${c} (${pix})` : String(c)
})

const inspectorFps = computed(() => {
    const w: any = props.selectedWallpaper
    const fps = w?.metadata?.fps
    logger.info('[详情] FPS 数据:', { wallpaper: w?.title, metadata: w?.metadata, fps })
    if (typeof fps !== 'number' || !Number.isFinite(fps)) return ''
    return `${fps.toFixed(2)} fps`
})

const inspectorBitRate = computed(() => {
    const w: any = props.selectedWallpaper
    const br = w?.metadata?.bit_rate
    if (typeof br !== 'number') return ''
    return formatBitRate(br)
})

const inspectorAudio = computed(() => {
    const w: any = props.selectedWallpaper
    const meta = w?.metadata
    if (!meta) return ''
    const codec = meta.audio_codec ? String(meta.audio_codec) : ''
    const sr = typeof meta.sample_rate === 'number' ? `${meta.sample_rate} Hz` : ''
    const ch = typeof meta.channels === 'number' ? `${meta.channels}` : ''
    if (!codec && !sr && !ch) return ''
    const parts = [codec, sr, ch ? `${ch}ch` : ''].filter(Boolean)
    return parts.join(' / ')
})

function getWallpaperType(w: any): string {
    if (w?.isVideo) return 'video'
    return 'image'
}

function getDirname(p: string): string {
    if (!p) return ''
    const s = p.replace(/[\\/]+$/, '')
    const idx = Math.max(s.lastIndexOf('\\'), s.lastIndexOf('/'))
    if (idx === -1) return ''
    return s.slice(0, idx)
}

async function handleApplySelected() {
    const now = Date.now()
    if (now - lastApplyAt < 400) return
    lastApplyAt = now

    const w: any = props.selectedWallpaper
    if (!w) return
    const id = w?.id ?? ''
    const path = w?.localPath ?? w?.path ?? ''
    if (!path || !id) return
    const type = getWallpaperType(w)
    const res = await api.wallpaperApply(id, path, type)
    if (!res.ok) {
        logger.error('应用壁纸失败:', res.error)
        if (res.error?.code === 'VIDEO_WALLPAPER_NOT_SUPPORTED') {
            showAlert?.(t('videoWallpaperNotSupported'))
        } else {
            showAlert?.(res.error?.message ?? t('applyFailed'))
        }
    } else {
        // Notify LibraryView to update current wallpaper state
        window.dispatchEvent(new CustomEvent('wallpaper:applied', { detail: { id } }))
    }
}

async function handleShowInFolder() {
    const p = inspectorFilePath.value
    if (!p) return
    const dir = getDirname(p)
    const target = dir || p
    const res = await api.systemOpenPath(target)
    if (!res.ok) {
        logger.error('打开路径失败:', res.error)
    }
}

async function handleRemoveSelected() {
    const w: any = props.selectedWallpaper
    if (!w?.id) return
    const res = await api.libraryRemove(w.id)
    if (!res.ok) {
        logger.error('删除壁纸失败:', res.error)
        return
    }
    emit('update:selectedWallpaper', null)
    window.dispatchEvent(new CustomEvent('library:changed'))
}

onMounted(async () => {
    const appWindow = getCurrentWindow()
    const unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
        isWindowVisible.value = focused
        const video = inspectorVideoRef.value
        if (!video) return

        if (!focused) {
            video.pause()
        } else if (props.selectedWallpaper?.isVideo) {
            video.play().catch(() => { })
        }
    })

    onUnmounted(() => {
        unlistenFocus()
    })
})

onUnmounted(() => {
    if (linuxRetryTimer) {
        clearTimeout(linuxRetryTimer)
        linuxRetryTimer = null
    }
    revokeObjectUrl(inspectorVideoSrc.value)
})

watch(
    () => props.selectedWallpaper,
    () => {
        const el = inspectorVideoRef.value
        if (!el) return
        try {
            el.pause()
            el.currentTime = 0
        } catch (e) {
            logger.warn('重置视频失败:', e)
        }

        nextTick(() => {
            const w: any = props.selectedWallpaper
            if (!w?.isVideo) return
            const v = inspectorVideoRef.value
            if (!v) return
            try {
                v.load()
                // Only play if window is visible
                if (isWindowVisible.value) {
                    v.play().catch(() => { })
                }
            } catch (e) {
                logger.warn('播放视频失败:', e)
            }
        })
    },
    { flush: 'post' }
)
</script>

<template>
    <div class="right-pane">
        <div v-if="!selectedWallpaper" class="inspector-empty">
            <NEmpty description="">
                <template #default>
                    <NText depth="3">{{ t('selectWallpaperHint') }}</NText>
                </template>
            </NEmpty>
        </div>

        <div v-else class="inspector-content">
            <div class="preview-large">
                <video v-if="(selectedWallpaper as any).isVideo" ref="inspectorVideoRef" :src="inspectorVideoSrc"
                    :poster="inspectorPosterSrc || undefined" muted loop playsinline preload="metadata" autoplay
                    @contextmenu.prevent
                    class="preview-media"
                    style="pointer-events: none" />
                <NImage v-else :src="(selectedWallpaper as any).thumbnail" :alt="(selectedWallpaper as any).title"
                    object-fit="cover" preview-disabled width="100%" height="100%" />
            </div>

            <NDivider style="margin: 12px 0" />

            <NScrollbar class="inspector-scroll">
                <NSpace vertical :size="12">
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('title') }}</NText>
                        <NInput :value="(selectedWallpaper as any).title || t('untitled')" size="small" />
                    </NSpace>
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('fileSize') }}</NText>
                        <NText>{{ inspectorFileSize || '-' }}</NText>
                    </NSpace>
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('resolution') }}</NText>
                        <NText>{{ inspectorResolution || '-' }}</NText>
                    </NSpace>
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('format') }}</NText>
                        <NText>{{ inspectorFormat || '-' }}</NText>
                    </NSpace>
                    <NSpace v-if="(selectedWallpaper as any).isVideo" vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('duration') }}</NText>
                        <NText>{{ inspectorDuration || '-' }}</NText>
                    </NSpace>
                    <NSpace v-if="(selectedWallpaper as any).isVideo" vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('videoCodec') }}</NText>
                        <NText>{{ inspectorVideoCodec || '-' }}</NText>
                    </NSpace>
                    <NSpace v-if="(selectedWallpaper as any).isVideo && inspectorFps" vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('fps') }}</NText>
                        <NText>{{ inspectorFps }}</NText>
                    </NSpace>
                    <NSpace v-if="(selectedWallpaper as any).isVideo && inspectorBitRate" vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('bitRate') }}</NText>
                        <NText>{{ inspectorBitRate }}</NText>
                    </NSpace>
                    <NSpace v-if="(selectedWallpaper as any).isVideo" vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('audioCodec') }}</NText>
                        <NText>{{ inspectorAudio || '-' }}</NText>
                    </NSpace>
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('importTime') }}</NText>
                        <NText>{{ inspectorImportTime || '-' }}</NText>
                    </NSpace>
                    <NSpace vertical :size="4">
                        <NText depth="3" style="font-size: 12px">{{ t('filePath') }}</NText>
                        <NText depth="3" style="font-size: 12px; word-break: break-all">
                            {{ inspectorFilePath || '-' }}
                        </NText>
                    </NSpace>
                </NSpace>
            </NScrollbar>

            <NDivider style="margin: 12px 0" />

            <div class="inspector-actions">
                <NSpace vertical :size="8">
                    <NButton block type="primary" @click="handleApplySelected">{{ t('applyWallpaper') }}</NButton>
                    <NButton block quaternary @click="handleShowInFolder">{{ t('showInFolder') }}</NButton>
                    <NButton block quaternary type="error" @click="handleRemoveSelected">{{ t('removeFromLibrary') }}</NButton>
                </NSpace>
            </div>
        </div>
    </div>
</template>

<style scoped>
.right-pane {
    width: 280px;
    min-width: 280px;
    height: calc(100vh - 48px);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: rgba(255, 255, 255, 0.02);
    border-left: 1px solid rgba(255, 255, 255, 0.06);
    position: fixed;
    right: 0;
    top: 48px;
}

.inspector-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
}

.inspector-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 16px;
}

.inspector-scroll {
    flex: 1;
    min-height: 0;
}

.preview-large {
    width: 100%;
    height: 150px;
    flex: 0 0 auto;
    position: relative;
    border-radius: 8px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.05);
    margin-bottom: 16px;
}

.preview-media {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.preview-large :deep(.n-image) {
    width: 100%;
    height: 100%;
}

.preview-large :deep(.n-image img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}
</style>
