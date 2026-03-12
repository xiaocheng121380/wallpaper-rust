import { defineStore } from "pinia";
import { watch } from "vue";
import { db } from "../utils/db";
import { logger } from '../utils/logger';

export type ThemeMode = "system" | "light" | "dark";

export interface SettingsState {
  language: string;
  themeMode: ThemeMode;
  primaryColor: string;
  _hydrated: boolean;
}

const defaultSettings: SettingsState = {
  language: "zh-CN",
  themeMode: "dark",
  primaryColor: "#18a058",
  _hydrated: false,
};

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    ...defaultSettings,
  }),
  actions: {
    async initialize() {
      try {
        const s = await db.get<{
          schema_version: number;
          language: string;
          theme_mode: string;
          primary_color?: string;
        }>("settings", {
          schema_version: 1,
          language: defaultSettings.language,
          theme_mode: defaultSettings.themeMode,
          primary_color: defaultSettings.primaryColor,
        });

        this.language = s.language === "en" ? "en" : defaultSettings.language;
        this.themeMode =
          (s.theme_mode as ThemeMode) || defaultSettings.themeMode;
        this.primaryColor = s.primary_color || defaultSettings.primaryColor;
      } finally {
        this._hydrated = true;
      }
    },

    async persist() {
      if (!this._hydrated) return;
      const payload = {
        schema_version: 1,
        language: this.language,
        theme_mode: this.themeMode,
        primary_color: this.primaryColor,
      };
      const ok = await db.set("settings", payload);
      if (!ok) {
        logger.warn('[设置] 保存失败')
      }
    },

    setLanguage(language: string) {
      this.language = language;
    },
    setThemeMode(mode: ThemeMode) {
      this.themeMode = mode;
    },
    setPrimaryColor(color: string) {
      this.primaryColor = color;
    },
    reset() {
      this.$patch(defaultSettings);
    },
  },
});

export function setupSettingsWatcher() {
  const store = useSettingsStore();
  watch(
    () => store.$state,
    () => {
      store.persist();
    },
    { deep: true },
  );
}
