<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { logger } from '../utils/logger'
import { listen } from '@tauri-apps/api/event'
import { NModal, NCard, NSpace, NRadio, NRadioGroup, NCheckbox, NButton, NText } from 'naive-ui'
import { api } from '../api'

const { t } = useI18n()

const showDialog = ref(false)
const selectedOption = ref<'minimize' | 'exit'>('minimize')
const rememberChoice = ref(true)

let unlisten: (() => void) | null = null

onMounted(async () => {
    unlisten = await listen('show-close-dialog', () => {
        showDialog.value = true
    })
})

onUnmounted(() => {
    if (unlisten) {
        unlisten()
    }
})

async function handleConfirm() {
    const minimizeToTray = selectedOption.value === 'minimize'

    showDialog.value = false

    // 调用后端命令处理首次关闭，后端会保存设置并执行相应操作
    const result = await api.windowHandleFirstClose(minimizeToTray, rememberChoice.value)
    if (!result.ok) {
        logger.error('处理首次关闭失败:', result.error)
    }
}
</script>

<template>
    <NModal v-model:show="showDialog" :mask-closable="false" :close-on-esc="false">
        <NCard style="width: 480px" :title="t('firstCloseTitle')" :bordered="false" size="large" role="dialog"
            aria-modal="true">
            <NSpace vertical :size="20">
                <NText>{{ t('firstCloseMessage') }}</NText>

                <NRadioGroup v-model:value="selectedOption">
                    <NSpace vertical :size="16">
                        <NRadio value="minimize">
                            <NSpace vertical :size="4">
                                <NText strong>{{ t('minimizeToTrayOption') }}</NText>
                                <NText depth="3" style="font-size: 12px">{{ t('minimizeToTrayDesc') }}</NText>
                            </NSpace>
                        </NRadio>

                        <NRadio value="exit">
                            <NSpace vertical :size="4">
                                <NText strong>{{ t('exitAppOption') }}</NText>
                                <NText depth="3" style="font-size: 12px">{{ t('exitAppDesc') }}</NText>
                            </NSpace>
                        </NRadio>
                    </NSpace>
                </NRadioGroup>

                <NCheckbox v-model:checked="rememberChoice">
                    {{ t('rememberChoice') }}
                </NCheckbox>

                <NButton type="primary" block @click="handleConfirm">
                    {{ t('confirm') }}
                </NButton>
            </NSpace>
        </NCard>
    </NModal>
</template>
