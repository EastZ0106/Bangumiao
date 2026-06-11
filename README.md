# bangumiao

Tauri 2.x 桌面番剧追番工具，整合蜜柑计划浏览、RSS 订阅、BT 下载和本地番剧库管理，Windows 本地运行。

## 功能

- **蜜柑计划内嵌浏览** — 原生 WebView 加载 mikanani.me，支持前进/后退/主页导航
- **RSS 一键订阅** — 扫描番剧详情页字幕组，预览剧集列表后一键订阅；订阅后自动抓取 RSS 开始下载
- **BT 下载管理** — aria2c 侧载（JSON-RPC over TCP），支持磁链 / 本地种子 / 远程 URL；实时进度轮询，暂停/恢复/删除
- **本地番剧库** — 递归扫描下载目录中的视频文件（mkv/mp4 等），智能解析文件名提取番剧标题和集数，按番剧分组，标记已看
- **定时刷新** — tokio 后台调度器按设定间隔自动拉取 RSS 发现新剧集
- **残留文件清理** — 一键递归清理所有子目录中的 `.torrent` / `.aria2` 中间文件
- **设置** — 下载目录、刷新间隔、aria2 端口、最大并发数、自动删除种子
- **SQLite 持久化** — WAL 模式，自动建表迁移，存储订阅/剧集/观看记录

## 技术栈

| 层 | 技术 |
|----|------|
| 前端 | React 19 + TypeScript + Vite + React Router 7 |
| 后端 | Rust (Tauri 2.x) + rusqlite + reqwest + quick-xml + tokio + chrono |
| 下载引擎 | aria2c (JSON-RPC over TCP，裸 HTTP 协议实现) |
| 存储 | SQLite (bundled, WAL mode) |

## 项目结构

```
bangumiao/
├── src/                          # React 前端
│   ├── components/Sidebar.tsx    # 侧边导航栏
│   ├── pages/
│   │   ├── Subscribe.tsx         # 订阅列表（首页）
│   │   ├── MikanBrowser.tsx      # 蜜柑计划内嵌浏览 + RSS 抓取
│   │   ├── Download.tsx          # 下载管理
│   │   ├── Library.tsx           # 本地番剧库
│   │   └── Settings.tsx          # 设置
│   └── styles/                   # 全局样式 & 主题变量
├── src-tauri/                    # Rust 后端
│   └── src/
│       ├── main.rs               # 入口（Windows 隐藏控制台）
│       ├── lib.rs                # AppState 初始化 + Tauri command 注册
│       ├── db.rs                 # SQLite 数据库 + 自动迁移
│       ├── aria2.rs              # aria2c JSON-RPC 客户端（裸 TCP/HTTP）
│       ├── rss_parser.rs         # RSS XML 解析 + 集数提取
│       ├── filename.rs           # 文件名智能解析（番剧标题 & 集数）
│       ├── scheduler.rs          # tokio 定时 RSS 刷新
│       └── commands/
│           ├── mikan.rs          # 蜜柑 WebView 操作 + RSS 扫描/抓取
│           ├── rss.rs            # 订阅增删改查 + 全量刷新
│           ├── download.rs       # 下载状态同步/暂停/恢复/删除/清理
│           ├── library.rs        # 本地视频扫描 + 标记已看
│           └── settings.rs       # 设置读写
├── download/                     # 默认下载目录（gitignore）
├── package.json                  # Node 依赖
├── pnpm-lock.yaml                # pnpm 锁文件
├── tsconfig.json                 # TypeScript 配置
└── vite.config.ts                # Vite 配置
```

## 开发

```bash
# 安装依赖
pnpm install

# 启动开发模式（Vite + Tauri）
pnpm tauri dev
```

aria2c 二进制文件需放在 `src-tauri/binaries/aria2c-x86_64-pc-windows-msvc.exe`。

## 构建

```bash
pnpm tauri build
```

输出在 `src-tauri/target/release/bundle/`。
