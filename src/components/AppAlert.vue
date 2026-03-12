<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { NModal, NCard, NSpace, NButton, NText } from 'naive-ui'

defineProps<{ show: boolean; message: string }>()

defineEmits<{ (e: 'update:show', value: boolean): void }>()

const { t } = useI18n()
</script>

<template>
  <NModal
    :show="show"
    :mask-closable="true"
    :close-on-esc="true"
    @update:show="$emit('update:show', $event)"
  >
    <NCard style="width: 420px" :bordered="false" size="large" role="dialog" aria-modal="true">
      <NSpace vertical :size="16">
        <NText strong>WallCraft</NText>
        <NText>{{ message }}</NText>
        <NButton type="primary" block @click="$emit('update:show', false)">
          {{ t('confirm') }}
        </NButton>
      </NSpace>
    </NCard>
  </NModal>
</template>
