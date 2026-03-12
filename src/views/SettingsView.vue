<script setup lang="ts">
defineOptions({ name: 'SettingsView' })

import { ref, computed, h, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore, type ThemeMode } from '../stores/settings'
import { api, type Settings, type ScreenResolution } from '../api'
import { logger } from '../utils/logger'
import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart'
import {
  NButton,
  NInputNumber,
  NSelect,
  NSwitch,
  NMenu,
  NLayout,
  NLayoutSider,
  NLayoutContent,
  NSpace,
  NText,
  NH2,
  NH3,
  NH4,
  NScrollbar,
  NDivider,
  NIcon,
  type MenuOption,
} from 'naive-ui'
import { ColorPaletteOutline, SettingsOutline, ConstructOutline, DesktopOutline, InformationCircleOutline, TrashOutline } from '@vicons/ionicons5'

const { t, locale } = useI18n()
const settingsStore = useSettingsStore()

const activeSection = ref('appearance')
const cacheSize = ref('0 B')
const backendSettings = ref<Settings | null>(null)
const screenResolution = ref<ScreenResolution | null>(null)

const vwMaxWidth = ref<number | null>(2560)
const vwMaxHeight = ref<number | null>(1440)
const vwFps = ref<number | null>(30)
const vwCrf = ref<number | null>(23)
const vwBitrateKbps = ref<number | null>(null)
const vwHwdec = ref(true)
const stopVideoOnExit = ref(true)
const minimizeToTray = ref(true)
const autoLaunch = ref(false)

const vwHydrated = ref(false)
let vwSaveTimer: number | null = null

function schedulePersistBackendSettings() {
  if (!vwHydrated.value) return
  if (vwSaveTimer !== null) {
    window.clearTimeout(vwSaveTimer)
  }
  vwSaveTimer = window.setTimeout(() => {
    persistBackendSettings().catch((e) => logger.error('保存设置失败:', e))
  }, 350)
}

function normalizeOptional(v: number | null): number | null {
  if (v === null) return null
  if (!Number.isFinite(v)) return null
  if (v <= 0) return null
  return v
}

type VideoPresetKey =
  | 'fhd_30'
  | 'fhd_60'
  | 'balanced'
  | 'smooth'
  | 'uhd_30'
  | 'uhd_60'
  | 'original'
  | 'custom'
const vwPreset = ref<VideoPresetKey>('balanced')

const vwPresetOptions = computed(() => [
  { label: '1080p / 30fps', value: 'fhd_30' },
  { label: '1080p / 60fps', value: 'fhd_60' },
  { label: t('presetBalanced'), value: 'balanced' },
  { label: t('presetSmooth'), value: 'smooth' },
  { label: t('presetUHD30'), value: 'uhd_30' },
  { label: t('presetUHD60'), value: 'uhd_60' },
  { label: t('presetOriginal'), value: 'original' },
  { label: t('presetCustom'), value: 'custom' },
])

function detectPreset(): VideoPresetKey {
  const w = vwMaxWidth.value
  const h = vwMaxHeight.value
  const fps = vwFps.value
  const crf = vwCrf.value
  const br = vwBitrateKbps.value

  if (br !== null) return 'custom'

  if (w === null && h === null && fps === null && crf === null) return 'original'
  if (w === 1920 && h === 1080 && fps === 30 && (crf === 23 || crf === 24 || crf === 26 || crf === 28)) return 'fhd_30'
  if (w === 1920 && h === 1080 && fps === 60 && (crf === 23 || crf === 24 || crf === 26 || crf === 28)) return 'fhd_60'
  if (w === 2560 && h === 1440 && fps === 30 && (crf === 23 || crf === 24)) return 'balanced'
  if (w === 2560 && h === 1440 && fps === 60 && (crf === 23 || crf === 24)) return 'smooth'
  if (w === 3840 && h === 2160 && fps === 30 && (crf === 21 || crf === 22 || crf === 23)) return 'uhd_30'
  if (w === 3840 && h === 2160 && fps === 60 && (crf === 21 || crf === 22 || crf === 23)) return 'uhd_60'
  return 'custom'
}

async function applyPreset(key: VideoPresetKey) {
  vwPreset.value = key
  if (key === 'fhd_30') {
    vwMaxWidth.value = 1920
    vwMaxHeight.value = 1080
    vwFps.value = 30
    vwCrf.value = 24
    vwBitrateKbps.value = null
  } else if (key === 'fhd_60') {
    vwMaxWidth.value = 1920
    vwMaxHeight.value = 1080
    vwFps.value = 60
    vwCrf.value = 24
    vwBitrateKbps.value = null
  } else if (key === 'balanced') {
    vwMaxWidth.value = 2560
    vwMaxHeight.value = 1440
    vwFps.value = 30
    vwCrf.value = 23
    vwBitrateKbps.value = null
  } else if (key === 'smooth') {
    vwMaxWidth.value = 2560
    vwMaxHeight.value = 1440
    vwFps.value = 60
    vwCrf.value = 23
    vwBitrateKbps.value = null
  } else if (key === 'uhd_30') {
    vwMaxWidth.value = 3840
    vwMaxHeight.value = 2160
    vwFps.value = 30
    vwCrf.value = 22
    vwBitrateKbps.value = null
  } else if (key === 'uhd_60') {
    vwMaxWidth.value = 3840
    vwMaxHeight.value = 2160
    vwFps.value = 60
    vwCrf.value = 22
    vwBitrateKbps.value = null
  } else if (key === 'original') {
    vwMaxWidth.value = null
    vwMaxHeight.value = null
    vwFps.value = null
    vwCrf.value = null
    vwBitrateKbps.value = null
  }

  if (key !== 'custom') {
    await persistBackendSettings()
  }
}

const menuOptions = computed<MenuOption[]>(() => [
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: ColorPaletteOutline }), t('appearance')]), key: 'appearance' },
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: SettingsOutline }), t('general')]), key: 'general' },
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: ConstructOutline }), t('advanced')]), key: 'advanced' },
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: DesktopOutline }), t('windowBehavior')]), key: 'window' },
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: TrashOutline }), t('cacheManagement')]), key: 'cache' },
  { label: () => h(NSpace, { align: 'center' }, () => [h(NIcon, { component: InformationCircleOutline }), t('about')]), key: 'about' },
])

function setLocale(next: string) {
  locale.value = next
  settingsStore.setLanguage(next)
  settingsStore.persist()
}

function setThemeMode(mode: ThemeMode) {
  settingsStore.setThemeMode(mode)
  settingsStore.persist()
}

function setPrimaryColor(color: string) {
  settingsStore.setPrimaryColor(color)
  settingsStore.persist()
}

async function refreshCacheSize() {
  const res = await api.cacheGetSize()
  if (!res.ok || res.data === undefined) {
    cacheSize.value = '0 B'
    return
  }
  const bytes = res.data
  if (bytes < 1024) {
    cacheSize.value = `${bytes} B`
  } else if (bytes < 1024 * 1024) {
    cacheSize.value = `${(bytes / 1024).toFixed(2)} KB`
  } else if (bytes < 1024 * 1024 * 1024) {
    cacheSize.value = `${(bytes / (1024 * 1024)).toFixed(2)} MB`
  } else {
    cacheSize.value = `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
  }
}

async function fetchScreenResolution() {
  const res = await api.systemGetScreenResolution()
  if (res.ok && res.data) {
    screenResolution.value = res.data
  }
}

async function handleClearCache() {
  await api.wallpaperStop()
  const res = await api.cacheClear()
  if (!res.ok) {
    logger.error('清理缓存失败:', res.error)
  }
  await refreshCacheSize()
}

async function handleOpenLogDir() {
  const res = await api.systemGetLogDir()
  if (!res.ok || !res.data) {
    logger.error('获取日志目录失败:', res.error)
    return
  }
  const logDir = res.data
  const openRes = await api.systemOpenPath(logDir)
  if (!openRes.ok) {
    logger.error('打开日志目录失败:', openRes.error)
  }
}

async function handleAutoLaunchToggle(enabled: boolean) {
  try {
    if (enabled) {
      await enable()
    } else {
      await disable()
    }
    autoLaunch.value = enabled
  } catch (e) {
    logger.error('切换开机自启失败:', e)
    // Revert the switch if failed
    autoLaunch.value = !enabled
  }
}

async function loadBackendSettings() {
  const res = await api.settingsGet()
  if (!res.ok || !res.data) {
    return
  }
  backendSettings.value = res.data

  const vw = res.data.video_wallpaper || {}
  vwMaxWidth.value = vw.max_width ?? 2560
  vwMaxHeight.value = vw.max_height ?? 1440
  vwFps.value = vw.fps ?? 30
  vwCrf.value = vw.crf ?? 23
  vwBitrateKbps.value = vw.bitrate_kbps ?? null
  vwHwdec.value = vw.hwdec ?? true
  stopVideoOnExit.value = res.data.stop_video_on_exit ?? true
  minimizeToTray.value = res.data.minimize_to_tray ?? true

  vwPreset.value = detectPreset()
  vwHydrated.value = true

  // Load auto-launch status
  try {
    autoLaunch.value = await isEnabled()
  } catch (e) {
    logger.error('检查开机自启状态失败:', e)
  }
}

async function persistBackendSettings() {
  if (!backendSettings.value) {
    return
  }

  const maxWidth = normalizeOptional(vwMaxWidth.value)
  const maxHeight = normalizeOptional(vwMaxHeight.value)
  const fps = normalizeOptional(vwFps.value)
  const crf = normalizeOptional(vwCrf.value)
  const bitrateKbps = normalizeOptional(vwBitrateKbps.value)

  const next: Settings = {
    ...backendSettings.value,
    video_wallpaper: {
      ...(backendSettings.value.video_wallpaper || {}),
      max_width: maxWidth,
      max_height: maxHeight,
      fps,
      crf,
      bitrate_kbps: bitrateKbps,
      hwdec: vwHwdec.value,
    },
    stop_video_on_exit: stopVideoOnExit.value,
    minimize_to_tray: minimizeToTray.value,
    first_close_handled: backendSettings.value.first_close_handled ?? false,
  }

  const res = await api.settingsUpdate(next)
  if (!res.ok || !res.data) {
    logger.error('更新设置失败:', res.error)
    return
  }
  backendSettings.value = res.data
  vwPreset.value = detectPreset()
}

watch([vwMaxWidth, vwMaxHeight, vwFps, vwCrf, vwBitrateKbps, vwHwdec, stopVideoOnExit, minimizeToTray], () => {
  vwPreset.value = detectPreset()
  schedulePersistBackendSettings()
})

onMounted(async () => {
  await loadBackendSettings()
  await refreshCacheSize()
  await fetchScreenResolution()
})

const presetColors = [
  '#f0a020',
  '#2080f0',
  '#18a058',
  '#d03050',
  '#722ed1',
  '#13c2c2',
  '#eb2f96',
  '#8a8a8a',
  '#6366f1',
  '#f472b6',
]

const languageOptions = [
  { label: '简体中文', value: 'zh-CN' },
  { label: 'English', value: 'en' },
]

const themeModes: { key: ThemeMode; label: string }[] = [
  { key: 'system', label: 'themeSystem' },
  { key: 'light', label: 'themeLight' },
  { key: 'dark', label: 'themeDark' },
]
</script>

<template>
  <NLayout class="settings-container" has-sider>
    <!-- 左侧菜单 -->
    <NLayoutSider :width="200" bordered content-style="padding: 16px 0;">
      <NH2 style="margin: 0 0 16px; padding: 0 16px">{{ t('settings') }}</NH2>
      <NMenu :value="activeSection" :options="menuOptions" @update:value="(key: string) => activeSection = key" />
    </NLayoutSider>

    <!-- 右侧内容 -->
    <NLayoutContent class="settings-content">
      <NSpace vertical style="padding: 20px">
        <NH3 style="margin: 0">{{ t(activeSection) }}</NH3>
        <NDivider style="margin: 12px 0" />

        <Transition name="section-fade" mode="out-in">
          <div :key="activeSection" class="section-wrap">
            <!-- 外观设置 -->
            <NScrollbar v-if="activeSection === 'appearance'" class="section-scroll"
              style="max-height: calc(100vh - 200px)">
              <NSpace vertical :size="16" style="padding-right: 20px">
                <NSpace justify="space-between" align="center">
                  <NText>{{ t('themeMode') }}</NText>
                  <NSelect class="motion-select" :value="settingsStore.themeMode"
                    :options="themeModes.map(m => ({ label: t(m.label), value: m.key }))" @update:value="setThemeMode"
                    @blur="settingsStore.persist()" style="width: 140px" size="small" />
                </NSpace>

                <NSpace justify="space-between" align="start">
                  <NText>{{ t('primaryColor') }}</NText>
                  <NSpace :size="8" :wrap="true" style="max-width: 280px">
                    <button v-for="color in presetColors" :key="color" class="color-btn"
                      :class="{ active: settingsStore.primaryColor === color }" :style="{ backgroundColor: color }"
                      @click="setPrimaryColor(color)" />
                  </NSpace>
                </NSpace>
              </NSpace>
            </NScrollbar>

            <!-- 通用设置 -->
            <NScrollbar v-else-if="activeSection === 'general'" class="section-scroll"
              style="max-height: calc(100vh - 200px)">
              <NSpace vertical :size="16" style="padding-right: 20px">
                <NSpace justify="space-between" align="center">
                  <NText>{{ t('language') }}</NText>
                  <NSelect class="motion-select" :value="locale" :options="languageOptions" @update:value="setLocale"
                    @blur="settingsStore.persist()" style="width: 140px" size="small" />
                </NSpace>
              </NSpace>
            </NScrollbar>

            <!-- 高级设置 -->
            <NScrollbar v-else-if="activeSection === 'advanced'" class="section-scroll"
              style="max-height: calc(100vh - 200px)">
              <NSpace vertical :size="16" style="padding-right: 20px">
                <NH4 style="margin: 0">{{ t('videoWallpaper') }}</NH4>
                <NText v-if="screenResolution" depth="3" style="font-size: 12px">
                  {{ t('currentScreen') }}：{{ screenResolution.width }}×{{ screenResolution.height }}
                </NText>
                <NSpace justify="space-between" align="center">
                  <NText>{{ t('preset') }}</NText>
                  <NSelect class="motion-select" :value="vwPreset" :options="vwPresetOptions" style="width: 200px"
                    size="small" @update:value="(v: any) => applyPreset(v as any)" />
                </NSpace>
                <NSpace justify="space-between" align="center">
                  <NText>{{ t('targetMaxWidth') }}</NText>
                  <NInputNumber v-model:value="vwMaxWidth" class="motion-select" size="small" :min="0" :step="10"
                    style="width: 160px" placeholder="2560" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('targetMaxHeight') }}</NText>
                  <NInputNumber v-model:value="vwMaxHeight" class="motion-select" size="small" :min="0" :step="10"
                    style="width: 160px" placeholder="1440" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('targetFPS') }}</NText>
                  <NInputNumber v-model:value="vwFps" class="motion-select" size="small" :min="1" :max="120" :step="1"
                    style="width: 160px" placeholder="30" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('quality') }}</NText>
                  <NInputNumber v-model:value="vwCrf" class="motion-select" size="small" :min="10" :max="40" :step="1"
                    style="width: 160px" placeholder="23" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('targetBitrate') }}</NText>
                  <NInputNumber v-model:value="vwBitrateKbps" class="motion-select" size="small" :min="0" :step="100"
                    style="width: 160px" :placeholder="t('targetBitratePlaceholder')" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('hardwareDecode') }}</NText>
                  <NSwitch v-model:value="vwHwdec" size="small" />
                </NSpace>

                <NDivider style="margin: 12px 0" />

                <NSpace justify="space-between" align="center">
                  <NSpace vertical :size="4">
                    <NText>{{ t('stopVideoOnExit') }}</NText>
                    <NText depth="3" style="font-size: 12px">{{ t('stopVideoOnExitHint') }}</NText>
                  </NSpace>
                  <NSwitch v-model:value="stopVideoOnExit" size="small" />
                </NSpace>
              </NSpace>
            </NScrollbar>

            <!-- 窗口行为 -->
            <NScrollbar v-else-if="activeSection === 'window'" class="section-scroll"
              style="max-height: calc(100vh - 200px)">
              <NSpace vertical :size="16" style="padding-right: 20px">
                <NSpace justify="space-between" align="center">
                  <NSpace vertical :size="4">
                    <NText>{{ t('minimizeToTray') }}</NText>
                    <NText depth="3" style="font-size: 12px">{{ t('minimizeToTrayHint') }}</NText>
                  </NSpace>
                  <NSwitch v-model:value="minimizeToTray" size="small" />
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NSpace vertical :size="4">
                    <NText>{{ t('autoLaunch') }}</NText>
                    <NText depth="3" style="font-size: 12px">{{ t('autoLaunchHint') }}</NText>
                  </NSpace>
                  <NSwitch :value="autoLaunch" size="small" @update:value="handleAutoLaunchToggle" />
                </NSpace>
              </NSpace>
            </NScrollbar>

            <!-- 缓存管理 -->
            <NScrollbar v-else-if="activeSection === 'cache'" class="section-scroll"
              style="max-height: calc(100vh - 200px)">
              <NSpace vertical :size="16" style="padding-right: 20px">
                <NSpace justify="space-between" align="center">
                  <NText>{{ t('cacheSize') }}</NText>
                  <NSpace align="center">
                    <NText depth="3">{{ cacheSize }}</NText>
                    <NButton class="motion-btn" size="small" @click="handleClearCache">
                      {{ t('clearCache') }}
                    </NButton>
                  </NSpace>
                </NSpace>

                <NSpace justify="space-between" align="center">
                  <NText>{{ t('openLogDir') }}</NText>
                  <NButton class="motion-btn" size="small" @click="handleOpenLogDir">
                    {{ t('openLogDir') }}
                  </NButton>
                </NSpace>
              </NSpace>
            </NScrollbar>

            <!-- 关于 -->
            <div v-else class="about-section">
              <NSpace vertical align="center" :size="20">
                <img src="/src/assets/logo.png" alt="WallCraft Logo"
                  style="width: 80px; height: 80px; border-radius: 16px;" />
                <NSpace vertical align="center" :size="4">
                  <NH3 style="margin: 0">{{ t('appName') }}</NH3>
                  <NText depth="3">v0.1.0</NText>
                </NSpace>
                <NText depth="2" style="text-align: center; max-width: 400px">
                  {{ t('aboutDescription') }}
                </NText>

                <NDivider style="margin: 12px 0; width: 300px" />

                <NSpace vertical align="start" :size="12" style="width: 100%; max-width: 400px">
                  <NH4 style="margin: 0">{{ t('aboutFeatures') }}</NH4>
                  <NSpace vertical :size="8">
                    <NText depth="2">• {{ t('aboutFeature1') }}</NText>
                    <NText depth="2">• {{ t('aboutFeature2') }}</NText>
                    <NText depth="2">• {{ t('aboutFeature3') }}</NText>
                    <NText depth="2">• {{ t('aboutFeature4') }}</NText>
                  </NSpace>
                </NSpace>

                <NDivider style="margin: 12px 0; width: 300px" />

                <NSpace vertical align="start" :size="12" style="width: 100%; max-width: 400px">
                  <NH4 style="margin: 0">{{ t('aboutPhilosophy') }}</NH4>
                  <NText depth="2" style="line-height: 1.6">
                    {{ t('aboutPhilosophyText') }}
                  </NText>
                </NSpace>
              </NSpace>
            </div>
          </div>
        </Transition>
      </NSpace>
    </NLayoutContent>
  </NLayout>
</template>

<style scoped>
.settings-container {
  height: 100%;
}

.settings-content {
  height: 100%;
  overflow: hidden;
}

.section-wrap {
  will-change: transform, opacity;
}

.section-fade-enter-active,
.section-fade-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.section-fade-enter-from,
.section-fade-leave-to {
  opacity: 0;
  transform: translateY(6px);
}

.section-scroll {
  border-radius: 10px;
}

.motion-btn {
  transition: transform 0.12s ease, filter 0.12s ease;
}

.motion-btn:hover {
  transform: translateY(-1px);
  filter: brightness(1.06);
}

.motion-btn:active {
  transform: translateY(0px) scale(0.98);
  filter: brightness(1.0);
}

.motion-select :deep(.n-base-selection) {
  transition: transform 0.12s ease, box-shadow 0.12s ease;
}

.motion-select :deep(.n-base-selection:hover) {
  transform: translateY(-1px);
}

.motion-select :deep(.n-base-selection) {
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.10);
  background: rgba(255, 255, 255, 0.04);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.22);
  backdrop-filter: blur(10px);
}

.motion-select :deep(.n-base-selection--active) {
  box-shadow:
    0 14px 40px rgba(0, 0, 0, 0.30),
    0 0 0 3px rgba(24, 160, 88, 0.22);
}

.motion-select :deep(.n-base-selection__border) {
  border: none;
}

.motion-select :deep(.n-base-selection-label) {
  padding: 0 10px;
}

.motion-select :deep(.n-base-selection-input__content) {
  font-weight: 600;
}

.motion-select :deep(.n-base-icon) {
  opacity: 0.8;
}

:deep(.n-base-select-menu) {
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(22, 22, 26, 0.92);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.50);
  backdrop-filter: blur(14px);
  overflow: hidden;
}

:deep(.n-base-select-option) {
  border-radius: 10px;
  margin: 4px;
  transition: background-color 0.12s ease, transform 0.12s ease;
}

:deep(.n-base-select-option:hover) {
  background: rgba(255, 255, 255, 0.08);
  transform: translateX(2px);
}

:deep(.n-base-select-option--selected) {
  background: rgba(24, 160, 88, 0.18);
}

:deep(.n-base-select-option--selected:hover) {
  background: rgba(24, 160, 88, 0.22);
}

.color-btn {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  transition: all 0.2s ease;
}

.color-btn:hover {
  transform: scale(1.1);
}

.color-btn.active {
  border-color: #fff;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.3);
  transform: scale(1.08);
}

.about-section {
  text-align: center;
  padding: 40px 20px;
}
</style>
