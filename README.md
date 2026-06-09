# bangumiao

Tauri 2.x + React + TypeScript 桌面番剧追番工具，Windows 本地运行。

## 功能

- **蜜柑计划内嵌浏览** — WebView 加载 mikanani.me，页内跳转详情
- **BT 下载管理** — aria2 侧载，支持磁链/种子/远程URL，实时进度
- **本地番剧库** — 扫描本地视频文件，按番剧名分组，标记已看
- **RSS 订阅管理** — 解析蜜柑计划 RSS，管理订阅增删
- **设置** — 下载目录、刷新间隔、aria2端口、最大并发数
- **SQLite 持久化存储** — 自动迁移

## 技术栈

- 前端：React 19 + TypeScript + Vite
- 后端：Rust (tauri 2.x) + rusqlite + reqwest + quick-xml + tokio
- 下载引擎：aria2c (JSON-RPC over TCP)

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 构建

```bash
pnpm tauri build
```
