import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface CommandResult<T> {
  ok: boolean;
  data?: T;
  error?: { code: string; message: string };
}

export interface MediaMetadata {
  width?: number;
  height?: number;
  format_name?: string;
  duration_sec?: number;
  bit_rate?: number;
  video_codec?: string;
  pix_fmt?: string;
  fps?: number;
  audio_codec?: string;
  sample_rate?: number;
  channels?: number;
}

export interface Wallpaper {
  id: string;
  title: string;
  local_path: string;
  thumbnail_path?: string;
  resolution?: string;
  file_size?: number;
  metadata?: MediaMetadata;
  import_time: string;
}

export interface Settings {
  schema_version: number;
  language: string;
  theme_mode: string;
  primary_color?: string;
  video_wallpaper?: {
    max_width?: number | null;
    max_height?: number | null;
    fps?: number | null;
    crf?: number | null;
    bitrate_kbps?: number | null;
    hwdec?: boolean | null;
  };
  current_wallpaper_id?: string | null;
  stop_video_on_exit?: boolean;
  minimize_to_tray?: boolean;
  first_close_handled?: boolean;
}

export interface ScreenResolution {
  width: number;
  height: number;
}

export interface DiscoverHistory {
  schema_version: number;
  urls: string[];
}

export type JsonValue = any;

let wallpaperApplyInFlight = false;
let wallpaperApplyQueued: {
  wallpaper_id: string;
  path: string;
  wallpaper_type: string;
  resolvers: Array<(r: CommandResult<boolean>) => void>;
} | null = null;

async function runWallpaperApply(
  wallpaper_id: string,
  path: string,
  wallpaper_type: string,
): Promise<CommandResult<boolean>> {
  wallpaperApplyInFlight = true;
  try {
    return await invoke("wallpaper_apply", {
      wallpaperId: wallpaper_id,
      path,
      wallpaperType: wallpaper_type,
    });
  } finally {
    wallpaperApplyInFlight = false;

    if (wallpaperApplyQueued) {
      const queued = wallpaperApplyQueued;
      wallpaperApplyQueued = null;
      runWallpaperApply(
        queued.wallpaper_id,
        queued.path,
        queued.wallpaper_type,
      ).then((res) => {
        queued.resolvers.forEach((resolve) => resolve(res));
      });
    }
  }
}

export const api = {
  async libraryList(): Promise<CommandResult<Wallpaper[]>> {
    return invoke("library_list");
  },

  async libraryGet(id: string): Promise<CommandResult<Wallpaper>> {
    return invoke("library_get", { id });
  },

  async libraryImport(paths: string[]): Promise<CommandResult<Wallpaper[]>> {
    return invoke("library_import", { paths });
  },

  async libraryRemove(id: string): Promise<CommandResult<boolean>> {
    return invoke("library_remove", { id });
  },

  async libraryUpdateTitle(
    id: string,
    title: string,
  ): Promise<CommandResult<Wallpaper>> {
    return invoke("library_update_title", { id, title });
  },

  async settingsGet(): Promise<CommandResult<Settings>> {
    return invoke("settings_get");
  },

  async settingsUpdate(settings: Settings): Promise<CommandResult<Settings>> {
    return invoke("settings_update", { settings });
  },

  async cacheGetSize(): Promise<CommandResult<number>> {
    return invoke("cache_get_size");
  },

  async cacheClear(): Promise<CommandResult<boolean>> {
    return invoke("cache_clear");
  },

  async discoverHistoryGet(): Promise<CommandResult<DiscoverHistory>> {
    return invoke("discover_history_get");
  },

  async discoverHistoryUpdate(
    history: DiscoverHistory,
  ): Promise<CommandResult<DiscoverHistory>> {
    return invoke("discover_history_update", { history });
  },

  async databaseJsonGet(file_name: string): Promise<CommandResult<JsonValue>> {
    return invoke("database_json_get", { fileName: file_name });
  },

  async databaseJsonSet(
    file_name: string,
    value: JsonValue,
  ): Promise<CommandResult<boolean>> {
    return invoke("database_json_set", { fileName: file_name, value });
  },

  async systemGetPlatform(): Promise<CommandResult<string>> {
    return invoke("system_get_platform");
  },

  async systemGetDataDir(): Promise<CommandResult<string>> {
    return invoke("system_get_data_dir");
  },

  async systemGetLogDir(): Promise<CommandResult<string>> {
    return invoke("system_get_log_dir");
  },

  async systemGetMediaBaseUrl(): Promise<CommandResult<string>> {
    return invoke("system_get_media_base_url");
  },

  // Window
  async windowHandleFirstClose(
    minimizeToTray: boolean,
    rememberChoice: boolean,
  ): Promise<CommandResult<boolean>> {
    return invoke("window_handle_first_close", {
      minimizeToTray,
      rememberChoice,
    });
  },

  async systemOpenPath(path: string): Promise<CommandResult<boolean>> {
    return invoke("system_open_path", { path });
  },

  async systemGetScreenResolution(): Promise<CommandResult<ScreenResolution>> {
    return invoke("system_get_screen_resolution");
  },

  async downloadUrlToDownloads(url: string): Promise<CommandResult<string>> {
    return invoke("download_url_to_downloads", { url });
  },

  async wallpaperApply(
    wallpaper_id: string,
    path: string,
    wallpaper_type: string,
  ): Promise<CommandResult<boolean>> {
    if (!wallpaperApplyInFlight) {
      return runWallpaperApply(wallpaper_id, path, wallpaper_type);
    }

    return new Promise<CommandResult<boolean>>((resolve) => {
      if (
        wallpaperApplyQueued &&
        wallpaperApplyQueued.wallpaper_id === wallpaper_id &&
        wallpaperApplyQueued.path === path &&
        wallpaperApplyQueued.wallpaper_type === wallpaper_type
      ) {
        wallpaperApplyQueued.resolvers.push(resolve);
        return;
      }
      wallpaperApplyQueued = {
        wallpaper_id,
        path,
        wallpaper_type,
        resolvers: [resolve],
      };
    });
  },

  async wallpaperStop(): Promise<CommandResult<boolean>> {
    return invoke("wallpaper_stop");
  },

  async thumbnailGet(id: string): Promise<CommandResult<string>> {
    return invoke("thumbnail_get", { id });
  },

  async getThumbnailBase64(
    id: string,
    sourcePath: string,
  ): Promise<CommandResult<string>> {
    return invoke("get_thumbnail_base64", { id, sourcePath });
  },

  async getThumbnailPath(
    id: string,
    sourcePath: string,
  ): Promise<CommandResult<string>> {
    return invoke("get_thumbnail_path", { id, sourcePath });
  },

  async getDetailPlayPath(
    id: string,
    sourcePath: string,
  ): Promise<CommandResult<string>> {
    return invoke("get_detail_play_path", { id, sourcePath });
  },

  async getVideoBase64(filePath: string): Promise<CommandResult<string>> {
    return invoke("get_video_base64", { filePath });
  },

  async getVideoBytes(filePath: string): Promise<CommandResult<number[]>> {
    return invoke("get_video_bytes", { filePath });
  },

  async openFileDialog(): Promise<string[] | null> {
    const result = await open({
      multiple: true,
      filters: [
        {
          name: "Media",
          extensions: [
            "jpg",
            "jpeg",
            "png",
            "gif",
            "bmp",
            "webp",
            "tiff",
            "ico",
            "svg",
            "mp4",
            "webm",
            "mkv",
            "avi",
            "mov",
            "wmv",
            "flv",
            "m4v",
          ],
        },
        {
          name: "Images",
          extensions: [
            "jpg",
            "jpeg",
            "png",
            "gif",
            "bmp",
            "webp",
            "tiff",
            "ico",
            "svg",
          ],
        },
        {
          name: "Videos",
          extensions: ["mp4", "webm", "mkv", "avi", "mov", "wmv", "flv", "m4v"],
        },
      ],
    });
    if (result === null) return null;
    if (Array.isArray(result)) return result;
    return [result];
  },

  async openFolderDialog(): Promise<string | null> {
    const result = await open({
      directory: true,
      multiple: false,
    });
    if (result === null) return null;
    if (Array.isArray(result)) return result[0];
    return result;
  },
};
