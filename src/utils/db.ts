import { api } from "../api";
import { logger } from "./logger";

async function writeDefault(fileName: string, value: unknown) {
  try {
    const result = await api.databaseJsonSet(fileName, value as any);
    if (!result.ok) {
      logger.warn("[数据库] 写入默认值失败", fileName, result.error);
    }
  } catch (e) {
    logger.warn("[数据库] 写入默认值异常", fileName, e);
  }
}

async function tryWriteDefault(fileName: string, value: unknown) {
  await writeDefault(fileName, value);
}

export const db = {
  async get<T>(fileName: string, defaultValue: T): Promise<T> {
    try {
      const result = await api.databaseJsonGet(fileName);
      if (!result.ok) return defaultValue;
      const v = result.data;
      if (v === null || typeof v === "undefined") {
        await tryWriteDefault(fileName, defaultValue);
        return defaultValue;
      }
      return v as T;
    } catch {
      await tryWriteDefault(fileName, defaultValue);
      return defaultValue;
    }
  },

  async set<T>(fileName: string, value: T): Promise<boolean> {
    try {
      const result = await api.databaseJsonSet(fileName, value as any);
      if (!result.ok) {
        logger.warn("[数据库] 写入失败", fileName, result.error);
      }
      return !!result.ok;
    } catch (e) {
      logger.warn("[数据库] 写入异常", fileName, e);
      return false;
    }
  },
};
