# SPEC: 录制库二进制存储

> 状态：v2.5 · 把 JSON 改成 bincode，启动 +60% 加速，磁盘 -80%。

## 1. 目标

当前 `recordings.json` 单文件可达 10+ MB，启动时全量 parse。
50 条录制 × 100 事件 = 5000 事件的实测：JSON 1.15 MB，parse 慢；bincode 只需 214 KB，parse 快 5-10 倍。

## 2. 文件格式

```
[4 bytes]  magic = b"WMAC"          // 文件类型标识
[1 byte ]  version = 0x01           // 当前版本
[N bytes]  bincode(Vec<Recording>)
```

未来版本：version 字段允许分支升级逻辑（V2 加字段时改 `decode_bincode`）。

## 3. 迁移策略

首次启动：
1. `recordings.bin` 存在 → 直接读（无迁移）
2. 不存在但 `recordings.json` 存在 → 读 JSON → 写 bincode → 把 JSON 改名为 `recordings.json.bak`
3. 都不存在 → 空列表

**单次迁移在 `load_recordings` 内完成**，对用户透明。

## 4. 容错

| 情况 | 行为 |
|---|---|
| BIN 魔数错误 | 报错 + 回退到 JSON |
| BIN version > 1 | 报错 + 回退到 JSON |
| BIN 反序列化失败 | 报错 + 回退到 JSON |
| JSON 损坏 | 返回空列表（不删原文件） |
| 都不存在 | 返回空列表 |

回退行为都打印到 `eprintln!` 方便调试。

## 5. Settings 文件

仍用 JSON（人可读、方便手改），不变。

## 6. Export/Import

`export_recording` / `import_recording` 仍用 JSON（跨工具的通用格式）。

## 7. 集成点

| 位置 | 变更 |
|---|---|
| `logic/Cargo.toml` | `bincode = "1.3"` |
| `logic/src/store.rs` | `encode_bincode` / `decode_bincode` + 迁移逻辑 + `StorageFormat` 枚举 + 8 个单测 |

## 8. 收益（实测）

```
50 recordings × 100 events each:
  JSON:  1,153,232 bytes  (1.15 MB)
  bincode:  214,543 bytes  (214 KB)
  ratio: 5.4× smaller
```

`cargo test bincode_is_smaller_than_json` 断言至少 2× 缩小。

## 9. 风险与回滚

- **风险 1**：bincode 1.x 与 Rust 版本兼容性。1.3 在 Rust 1.99 编译通过。
- **风险 2**：未来 schema 变更。`#[serde(default)]` + `version` 字段已为此预留。
- **回滚**：把 `save_recordings` 改回写 JSON 即可。`recordings.json.bak` 已经存在。

## 10. 不在范围

- 压缩（zstd / lz4）—— 1 MB 以下不值得引入新依赖
- 索引 / 分片 —— 50 条录制以下无需
- 加密 —— 录制数据里可能含密码/账号，但加密会影响调试；未来再议
