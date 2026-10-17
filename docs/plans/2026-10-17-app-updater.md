# WinMacro 应用内自动更新（App Updater）实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 WinMacro v2（Tauri 2）添加完整的应用内更新链路：检查更新 → 展示版本与更新说明 → 下载（进度条）→ 重启并静默安装，数据源为 GitHub Releases。

**Architecture:** 采用 Tauri 官方更新链路 —— `tauri-plugin-updater`（检查/下载/验签/安装）+ `tauri-plugin-process`（重启）。CI 侧由 tauri-action 在打 tag 时生成签名安装包与 `latest.json` 清单并附到 Release；客户端按 semver 比较版本，下载后用 minisign 公钥验签，Windows 上 NSIS 静默安装（`/S`，currentUser 模式无 UAC）。

**Tech Stack:** Tauri 2（tauri-plugin-updater 2 / tauri-plugin-process 2）、Vue 3 前端（@tauri-apps/plugin-updater / plugin-process）、GitHub Actions secrets（libsodium sealed box 加密写入）。

---

## 背景与现状（工程师需知）

| 文件 | 现状 | 本计划动作 |
|---|---|---|
| `app-tauri/src-tauri/tauri.conf.json` | version 2.0.0，无 `plugins` 键，bundle 无 updater 配置 | 加 `createUpdaterArtifacts` + `plugins.updater`，bump 2.1.0 |
| `app-tauri/src-tauri/src/main.rs:144` | `tauri::Builder::default().plugin(shortcuts)` | 追加两个插件注册 |
| `app-tauri/src-tauri/capabilities/default.json` | `permissions: ["core:default"]` | 加 `updater:default`、`process:allow-restart` |
| `app-tauri/src-tauri/Cargo.toml` | tauri 2 / global-shortcut 2 | 加 updater/process 两依赖 |
| `app-tauri/logic/src/model.rs:136` | `Settings` 结构体 6 字段 | 加 `auto_check_updates: bool`（TDD） |
| `app-tauri/src/api.ts` | 现有 api 对象 + `onEvent` | 加 updater 封装函数 |
| `app-tauri/src/views/Settings.vue` | 设置页（录制/执行区块） | 加「应用更新」区块 |
| `app-tauri/src/App.vue` | 侧栏 + Library/Settings 切换 | 挂 UpdateDialog + 启动自动检查 |
| `.github/workflows/tauri.yml` | tauri-action 已出 NSIS | env 注入两个签名 secrets |
| GitHub Secrets | 空 | `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |

**关键事实：**
- 仓库 `plxlaaa/winmacro` 是 public → `latest.json` 无 token 即可下载，endpoint 用 `https://github.com/plxlaaa/winmacro/releases/latest/download/latest.json`。
- **v2.0.0（用户已装的版本）不含 updater 插件** → v2.1.0 需手动安装一次，之后所有版本在线更新。
- NSIS `installMode: "currentUser"` 已配置 → 静默更新不触发 UAC。
- 私钥丢失 = 已发布版本永远无法在线升级（只能换密钥对让用户手动装）→ **私钥必须妥善备份**（本计划存 `/home/z/.tauri/`，交付时提醒用户拷贝）。
- 沙箱无法编译 src-tauri（缺 webkit2gtk 系统库）→ Rust 侧验证依赖 CI；本地可验证：logic 单测、vue-tsc 严格检查、vite build、JSON 语法。

---

### Task 1: 生成更新签名密钥对 + 写入 GitHub Secrets

**Files:**
- Create: `/home/z/.tauri/winmacro-updater.key`（私钥，仓库外，永不入 git）
- Create: `/home/z/.tauri/winmacro-updater.key.password`（密码）
- Create: `/home/z/.tauri/winmacro-updater.key.pub`（公钥，内容稍后写入 tauri.conf.json）
- Create: `/tmp/set_secrets.py`（一次性脚本，跑完即删）

**Step 1: 生成密钥对（非交互）**

```bash
mkdir -p /home/z/.tauri
cd /home/z/my-project/winmacro/app-tauri
PW=$(openssl rand -base64 18)
echo -n "$PW" > /home/z/.tauri/winmacro-updater.key.password
npx tauri signer generate -w /home/z/.tauri/winmacro-updater.key -p "$PW"
ls -la /home/z/.tauri/
```

Expected: 输出包含 `Your secret key was generated successfully`，目录下有 `.key` / `.key.pub` / `.key.password` 三个文件。

**Step 2: 读取公钥（Task 2 要用）**

```bash
cat /home/z/.tauri/winmacro-updater.key.pub
```

Expected: 一行 base64（形如 `dW50cnVzdGVkIGNvbW1lbnQ6...`）。

**Step 3: 安装 pynacl 并写 secrets 上传脚本**

```bash
python3 -m venv /tmp/nacl-venv && /tmp/nacl-venv/bin/pip install -q pynacl requests
```

`/tmp/set_secrets.py`（token 从环境变量读，不落盘到脚本）：

```python
import base64, os, requests
from nacl import encoding, public

REPO = "plxlaaa/winmacro"
h = {"Authorization": f"token {os.environ['GH_TOKEN']}", "Accept": "application/vnd.github+json"}
key = requests.get(f"https://api.github.com/repos/{REPO}/actions/public-key", headers=h).json()
pk = public.PublicKey(key["key"].encode(), encoding.Base64Encoder())

def put(name, value):
    sealed = pk.seal(value.encode())
    r = requests.put(
        f"https://api.github.com/repos/{REPO}/actions/secrets/{name}",
        headers=h,
        json={"encrypted_value": base64.b64encode(sealed).decode(), "key_id": key["key_id"]},
    )
    print(name, "->", r.status_code)

put("TAURI_SIGNING_PRIVATE_KEY", open("/home/z/.tauri/winmacro-updater.key").read())
put("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", open("/home/z/.tauri/winmacro-updater.key.password").read().strip())
```

**Step 4: 执行并验证**

```bash
export GH_TOKEN=<token 从 git remote 现取现用>
/tmp/nacl-venv/bin/python /tmp/set_secrets.py
curl -s -H "Authorization: token $GH_TOKEN" "https://api.github.com/repos/plxlaaa/winmacro/actions/secrets" | python3 -c "import json,sys; [print(s['name']) for s in json.load(sys.stdin)['secrets']]"
rm /tmp/set_secrets.py
```

Expected: 两行 `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。

**Commit:** 无（密钥不入 git）。

---

### Task 2: Rust 侧接入 updater / process 插件

**Files:**
- Modify: `app-tauri/src-tauri/Cargo.toml`（[dependencies] 尾部）
- Modify: `app-tauri/src-tauri/src/main.rs:144-146`
- Modify: `app-tauri/src-tauri/tauri.conf.json`（bundle 内 + 顶层 plugins）
- Modify: `app-tauri/src-tauri/capabilities/default.json`

**Step 1: Cargo.toml 加依赖**

```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

**Step 2: main.rs 注册插件（紧跟 `.plugin(shortcuts)` 之后）**

```rust
    tauri::Builder::default()
        .plugin(shortcuts)
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::default())
```

**Step 3: tauri.conf.json 三处改动**

`bundle` 对象内加一行（`"active": true,` 之后）：

```json
    "createUpdaterArtifacts": true,
```

顶层（`bundle` 平级）新增：

```json
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/plxlaaa/winmacro/releases/latest/download/latest.json"
      ],
      "pubkey": "<Task 1 Step 2 读到的公钥整行粘贴>"
    }
  },
```

**Step 4: capabilities 加权限**

```json
  "permissions": ["core:default", "updater:default", "process:allow-restart"]
```

**Step 5: 验证 JSON 语法 + 提交**

```bash
cd /home/z/my-project/winmacro
python3 -c "import json; json.load(open('app-tauri/src-tauri/tauri.conf.json')); json.load(open('app-tauri/src-tauri/capabilities/default.json')); print('JSON-OK')"
git add app-tauri/src-tauri
git commit -m "feat(updater): register updater & process plugins with signed-release config"
```

Expected: `JSON-OK`；Rust 编译验证留给 CI（沙箱缺 webkit2gtk）。

---

### Task 3: Settings 模型加 auto_check_updates（TDD）

**Files:**
- Modify: `app-tauri/logic/src/model.rs`（Settings 结构体 + Default）
- Test: `app-tauri/logic/src/model.rs`（tests 模块内追加）

**Step 1: 写失败测试（model.rs 的 `mod tests` 内追加）**

```rust
    #[test]
    fn settings_auto_check_updates_roundtrip() {
        let s = Settings::default();
        assert!(s.auto_check_updates);

        let raw: Settings =
            serde_json::from_str(r#"{"auto_check_updates":false}"#).unwrap();
        assert!(!raw.auto_check_updates);

        let text = serde_json::to_string(&Settings::default()).unwrap();
        assert!(text.contains("\"auto_check_updates\":true"));
    }
```

**Step 2: 跑测试确认失败**

```bash
cd /home/z/my-project/winmacro/app-tauri/logic && cargo test
```

Expected: FAIL `no field auto_check_updates`（编译错误即失败）。

**Step 3: 最小实现**

结构体（`record_mouse_move` 之后）：

```rust
    /// 启动后是否自动检查应用更新（设置页可关）。
    pub auto_check_updates: bool,
```

`Default` 实现加：

```rust
            auto_check_updates: true,
```

**Step 4: 跑测试确认通过**

```bash
cargo test
```

Expected: `test result: ok. 18 passed`（原 17 + 新 1）。

**Step 5: Commit**

```bash
git add app-tauri/logic/src/model.rs
git commit -m "feat(updater): auto_check_updates setting (default on)"
```

---

### Task 4: 前端更新 UI（状态模块 + 对话框 + 设置区块 + 启动检查）

**Files:**
- Create: `app-tauri/src/update.ts`
- Create: `app-tauri/src/components/UpdateDialog.vue`
- Modify: `app-tauri/src/api.ts`（Settings interface 加字段）
- Modify: `app-tauri/src/views/Settings.vue`
- Modify: `app-tauri/src/App.vue`

**Step 1: 安装前端插件包**

```bash
cd /home/z/my-project/winmacro/app-tauri
npm install @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

**Step 2: api.ts — Settings 接口加字段（与 Task 3 对齐）**

```ts
export interface Settings {
  resolution_mode: string;
  screen_width: number;
  screen_height: number;
  wait_seconds: number;
  hide_on_play: boolean;
  record_mouse_move: boolean;
  auto_check_updates: boolean;
}
```

**Step 3: 新建 src/update.ts（模块级响应式状态 + 动作）**

```ts
import { reactive } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "uptodate"
  | "error";

export const updateState = reactive({
  phase: "idle" as UpdatePhase,
  currentVersion: "",
  newVersion: "",
  notes: "",
  downloaded: 0,
  total: 0,
  error: "",
});

let current: Update | null = null;

export async function checkForUpdates(): Promise<void> {
  if (updateState.phase === "checking" || updateState.phase === "downloading") return;
  updateState.phase = "checking";
  updateState.error = "";
  try {
    updateState.currentVersion = await getVersion();
    current = await check();
    if (current) {
      updateState.newVersion = current.version;
      updateState.notes = current.body ?? "";
      updateState.phase = "available";
    } else {
      updateState.phase = "uptodate";
    }
  } catch (e) {
    updateState.phase = "error";
    updateState.error = String(e);
  }
}

export async function downloadAndInstall(): Promise<void> {
  if (!current) return;
  updateState.phase = "downloading";
  updateState.downloaded = 0;
  updateState.total = 0;
  updateState.error = "";
  try {
    await current.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          updateState.total = event.data.contentLength ?? 0;
          break;
        case "Progress":
          updateState.downloaded += event.data.chunkLength;
          break;
        case "Finished":
          updateState.phase = "ready";
          break;
      }
    });
    updateState.phase = "ready";
  } catch (e) {
    updateState.phase = "error";
    updateState.error = String(e);
  }
}

export async function relaunchApp(): Promise<void> {
  await relaunch();
}

export function dismissUpdate(): void {
  if (updateState.phase !== "downloading") {
    updateState.phase = "idle";
    current = null;
  }
}
```

**Step 4: 新建 src/components/UpdateDialog.vue**

```vue
<script setup lang="ts">
import { computed } from "vue";
import {
  updateState,
  downloadAndInstall,
  relaunchApp,
  dismissUpdate,
} from "../update";

const visible = computed(
  () =>
    updateState.phase === "available" ||
    updateState.phase === "downloading" ||
    updateState.phase === "ready" ||
    updateState.phase === "error",
);
const pct = computed(() =>
  updateState.total > 0
    ? Math.min(100, Math.round((updateState.downloaded / updateState.total) * 100))
    : 0,
);
const sizeText = computed(
  () =>
    `${(updateState.downloaded / 1048576).toFixed(1)} MB` +
    (updateState.total > 0 ? ` / ${(updateState.total / 1048576).toFixed(1)} MB` : ""),
);
</script>

<template>
  <div v-if="visible" class="upd-mask">
    <div class="upd-card">
      <template v-if="updateState.phase === 'available'">
        <div class="upd-title">发现新版本 v{{ updateState.newVersion }}</div>
        <div class="upd-sub">当前 v{{ updateState.currentVersion }}</div>
        <pre class="upd-notes">{{ updateState.notes || "（本次更新没有附加说明）" }}</pre>
        <div class="upd-actions">
          <button class="btn" @click="dismissUpdate">暂不更新</button>
          <button class="btn primary" @click="downloadAndInstall">立即更新</button>
        </div>
      </template>

      <template v-else-if="updateState.phase === 'downloading'">
        <div class="upd-title">正在下载 v{{ updateState.newVersion }}</div>
        <div class="upd-bar"><div class="upd-fill" :style="{ width: pct + '%' }"></div></div>
        <div class="upd-sub">{{ pct }}%（{{ sizeText }}）… 请勿关闭应用</div>
      </template>

      <template v-else-if="updateState.phase === 'ready'">
        <div class="upd-title">更新已就绪</div>
        <div class="upd-sub">重启应用后自动完成安装（静默，无需操作）。</div>
        <div class="upd-actions">
          <button class="btn primary" @click="relaunchApp">重启并安装</button>
        </div>
      </template>

      <template v-else>
        <div class="upd-title">更新失败</div>
        <pre class="upd-notes">{{ updateState.error }}</pre>
        <div class="upd-actions">
          <button class="btn" @click="dismissUpdate">关闭</button>
          <button class="btn primary" @click="downloadAndInstall">重试</button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.upd-mask {
  position: fixed; inset: 0; z-index: 300;
  background: rgba(4, 8, 14, 0.72);
  display: grid; place-items: center;
}
.upd-card {
  width: min(520px, 90vw);
  background: var(--card);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius);
  padding: 26px 28px;
  display: flex; flex-direction: column; gap: 14px;
}
.upd-title { font-size: 17px; font-weight: 700; }
.upd-sub { color: var(--muted); font-size: 13px; }
.upd-notes {
  max-height: 220px; overflow: auto;
  background: var(--panel); border: 1px solid var(--border);
  border-radius: var(--radius-sm); padding: 12px 14px;
  font-size: 12.5px; line-height: 1.7; color: var(--muted);
  white-space: pre-wrap; word-break: break-word;
}
.upd-bar {
  height: 10px; border-radius: 6px;
  background: var(--panel); border: 1px solid var(--border-strong);
  overflow: hidden;
}
.upd-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--accent), #60a5fa);
  transition: width 0.2s ease;
}
.upd-actions { display: flex; justify-content: flex-end; gap: 10px; }
</style>
```

**Step 5: App.vue — 挂对话框 + 启动自动检查**

`<script setup>` 顶部 import 区追加：

```ts
import UpdateDialog from "./components/UpdateDialog.vue";
import { checkForUpdates } from "./update";
```

`onMounted` 内（`refreshFlags();` 之后）追加：

```ts
  // 启动 3 秒后按设置静默检查更新
  setTimeout(async () => {
    try {
      const s = await api.getSettings();
      if (s.auto_check_updates) await checkForUpdates();
    } catch {
      /* 静默失败，不打扰用户 */
    }
  }, 3000);
```

模板尾部（`</main>` 之后、根 `</div>` 之前）：

```html
    <UpdateDialog />
```

**Step 6: Settings.vue — 「应用更新」区块**

`<script setup>` import 区追加：

```ts
import { updateState, checkForUpdates } from "../update";
import { getVersion } from "@tauri-apps/api/app";
```

状态区追加：

```ts
const appVersion = ref("");
const updMsg = ref("");

async function checkNow() {
  updMsg.value = "正在检查…";
  await checkForUpdates();
  if (updateState.phase === "uptodate") updMsg.value = "✓ 已是最新版本";
  else if (updateState.phase === "available") updMsg.value = ""; // 对话框已弹出
  else if (updateState.phase === "error") updMsg.value = "检查失败：" + updateState.error;
}
```

`onMounted` 内追加：

```ts
  appVersion.value = await getVersion().catch(() => "?");
```

设置容器内（「快捷键」`form-row` 之后）追加：

```html
      <div class="form-row">
        <div>
          <div class="form-label">应用更新</div>
          <div class="form-hint">当前版本 v{{ appVersion }}。启动时自动检查新版本。</div>
        </div>
        <div style="display: flex; align-items: center; gap: 14px">
          <label class="toggle-row" style="margin: 0">
            <input v-model="s.auto_check_updates" type="checkbox" class="tgl" />
            <span>自动检查</span>
          </label>
          <button class="btn" @click="checkNow">检查更新</button>
          <span v-if="updMsg" style="color: var(--muted); font-size: 12.5px">{{ updMsg }}</span>
        </div>
      </div>
```

（若 `toggle-row`/`tgl` 样式类不存在，复用现有开关行同款 class；以现有 hide_on_play 行的写法为准。）

**Step 7: 验证 + 提交**

```bash
cd /home/z/my-project/winmacro/app-tauri
npm run build
```

Expected: `vue-tsc` 无错误，vite 构建成功。

```bash
cd /home/z/my-project/winmacro
git add app-tauri/src app-tauri/package.json app-tauri/package-lock.json
git commit -m "feat(updater): in-app update flow with progress dialog and auto-check toggle"
```

---

### Task 5: 版本 bump + README + workflow 注入签名 secrets

**Files:**
- Modify: `app-tauri/src-tauri/tauri.conf.json:4`（"version": "2.1.0"）
- Modify: `app-tauri/package.json:4`（"version": "2.1.0"）
- Modify: `README.md`（功能列表加一行）
- Modify: `.github/workflows/tauri.yml`（tauri-action env）

**Step 1: 两个 version 改为 `2.1.0`。**

**Step 2: README 功能列表追加：**

```markdown
- **应用内更新**：启动时自动检查 GitHub Release 新版本，一键下载安装（带进度与更新说明），详见设置页
```

**Step 3: tauri.yml 的 tauri-action step 加 env：**

```yaml
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

**Step 4: 验证 + 提交**

```bash
python3 -c "import json; c=json.load(open('app-tauri/src-tauri/tauri.conf.json')); assert c['version']=='2.1.0'; print('OK')"
git add -A
git commit -m "chore(release): v2.1.0 with updater artifacts and CI signing secrets"
```

---

### Task 6: 发布与验收

**Step 1: 推送 + 打 tag**

```bash
git push origin master
git tag -a v2.1.0 -m "WinMacro 2.1.0 — in-app auto-update" && git push origin v2.1.0
```

**Step 2: 盯梢 Tauri Release 构建（历史耗时 ~2.5h，job 上限 6h）**

```bash
curl -s -H "Authorization: token <TOKEN>" \
  "https://api.github.com/repos/plxlaaa/winmacro/actions/runs?event=push&per_page=1"
```

Expected: 最终 `conclusion: success`。

**Step 3: 验收 Release 附件**

```bash
curl -s "https://api.github.com/repos/plxlaaa/winmacro/releases/latest" | \
  python3 -c "import json,sys; r=json.load(sys.stdin); print(r['tag_name']); [print(' ', a['name'], f\"{a['size']/1048576:.1f}MB\") for a in r['assets']]"
```

Expected: 附件含 `WinMacro_2.1.0_x64-setup.exe` **和 `latest.json`**（tauri-action 在 createUpdaterArtifacts + 签名 env 就位时自动生成上传）。

**Step 4: 验收 latest.json 内容**

```bash
curl -sL "https://github.com/plxlaaa/winmacro/releases/latest/download/latest.json" | python3 -m json.tool
```

Expected: `version: "2.1.0"`、`pubkey` 与 tauri.conf.json 一致、`platforms["windows-x86_64"].signature` 非空。

**Step 5: 下载 setup.exe 发用户 + 更新记忆（daily + 长期记忆）+ 交付说明**

交付说明必须包含：① v2.0.0 → v2.1.0 需手动装一次；② 私钥位置 `/home/z/.tauri/winmacro-updater.key(.password)` 请自行备份到本地；③ 之后新版本应用内一键更新。

---

### Task 7（可选，建议做）: 真实更新链路 E2E

发一个只 bump 版本的 v2.1.1（如 v2.1.1 + 一行 release note），让装了 v2.1.0 的客户端真实走完「发现 → 下载 → 静默安装 → 重启进新版」全链。成本：一次 ~2.5h CI 构建。**需用户确认后执行。**

---

## 风险清单

| 风险 | 缓解 |
|---|---|
| 私钥丢失 → 老版本永久无法在线升级 | 私钥存 `/home/z/.tauri/`（仓库外），交付时明确提醒用户备份 |
| tauri-action 未生成 latest.json | Task 6 Step 3 显式验收；若无则检查 secrets 是否在 tag 构建前就位 |
| `updater:default` 权限名写错 → 前端 check() 被拒 | capabilities 用官方权限标识符；CI 编译 + 本地 vue-tsc 双保险 |
| 用户已装 v2.0.0 无法收到任何推送 | 交付说明明确：v2.1.0 需手动装一次 |
| Release notes 含 HTML → 对话框显示原始标签 | update.notes 用 `pre-wrap` 纯文本展示 |
| settings.json 旧数据无 auto_check_updates 字段 | `#[serde(default)]` 已在 Settings 上，bool 默认 false —— 注意：serde default 走 Default::default() 的 false 而非我们 Default impl 的 true，旧用户首次读到 false，保存一次后变 true。可接受（不打扰老用户首次启动），README 不必展开 |

## 验收清单（全部满足才算完成）

- [ ] GitHub Secrets 两个已写入
- [ ] logic 单测 18 全绿（含 auto_check_updates）
- [ ] vue-tsc + vite build 通过
- [ ] CI 构建成功，Release 含 setup.exe + latest.json
- [ ] latest.json 的 version / pubkey / signature 三要素正确
- [ ] 安装器已发用户，交付说明含私钥备份提醒
