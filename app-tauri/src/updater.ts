import { ref } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";

export interface UpdateInfo {
  version: string;
  body: string;
}

// Update 是带资源句柄的类实例（Rust 侧持有 rid），不放入响应式系统；
// 模块内私有持有，对外只暴露纯数据 UpdateInfo。
let currentUpdate: Update | null = null;

export const updateInfo = ref<UpdateInfo | null>(null);
export const updateChecking = ref(false);
export const updateError = ref("");
export const updatePhase = ref<"idle" | "downloading" | "ready">("idle");
export const updateProgress = ref<{ received: number; total: number } | null>(null);
export const showUpdateDialog = ref(false);

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  updateChecking.value = true;
  updateError.value = "";
  try {
    const u = await check();
    currentUpdate = u;
    updateInfo.value = u ? { version: u.version, body: u.body ?? "" } : null;
    return updateInfo.value;
  } catch (e) {
    updateError.value = String(e);
    return null;
  } finally {
    updateChecking.value = false;
  }
}

export async function downloadUpdate(): Promise<void> {
  if (!currentUpdate) return;
  updatePhase.value = "downloading";
  updateProgress.value = null;
  let received = 0;
  let total = 0;
  try {
    await currentUpdate.downloadAndInstall((ev) => {
      switch (ev.event) {
        case "Started":
          total = ev.data.contentLength ?? 0;
          break;
        case "Progress":
          received += ev.data.chunkLength;
          break;
        case "Finished":
          updatePhase.value = "ready";
          break;
      }
      updateProgress.value = { received, total };
    });
    updatePhase.value = "ready";
  } catch (e) {
    updateError.value = String(e);
    updatePhase.value = "idle";
    throw e;
  }
}

export function fmtMB(bytes: number): string {
  return (bytes / 1048576).toFixed(1) + " MB";
}
