import PageIllustration from "../components/PageIllustration";

export default function Help() {
  const sectionStyle: React.CSSProperties = {
    marginBottom: 24,
    padding: "16px 20px",
    borderRadius: 10,
    border: "1px solid var(--border-color)",
    background: "var(--bg-card)",
  };

  const h2Style: React.CSSProperties = {
    fontSize: 16, fontWeight: 600, marginBottom: 10,
  };

  const h3Style: React.CSSProperties = {
    fontSize: 14, fontWeight: 600, marginTop: 14, marginBottom: 6,
  };

  const pStyle: React.CSSProperties = {
    fontSize: 13, lineHeight: 1.8, color: "var(--text-secondary)",
  };

  const codeStyle: React.CSSProperties = {
    background: "var(--bg-sidebar)", padding: "1px 6px", borderRadius: 4,
    fontSize: 12, fontFamily: "JetBrains Mono, Consolas, monospace",
  };

  return (
    <div style={{ maxWidth: 760 }}>
      <div className="page-header">
        <div className="page-header-left">
          <PageIllustration page="/help" />
          <div>
            <h1 className="page-title">帮助</h1>
            <p className="page-subtitle">bangumiao 用户手册</p>
          </div>
        </div>
      </div>

      {/* 1. Subscription */}
      <div style={sectionStyle}>
        <h2 style={h2Style}>📡 订阅管理</h2>
        <p style={pStyle}>
          bangumiao 通过 RSS 订阅追踪蜜柑计划（mikanani.me）上的番剧更新。在侧边栏点击「蜜柑计划」进入浏览器，浏览到感兴趣的番剧详情页，点击「抓取 RSS」按钮即可扫描该番剧下所有可用字幕组。
        </p>
        <h3 style={h3Style}>添加订阅</h3>
        <p style={pStyle}>
          1. 在蜜柑计划浏览器中打开番剧详情页（URL 包含 <span style={codeStyle}>/Home/Bangumi/数字</span>）<br/>
          2. 点击右上角「抓取 RSS」→ 选择字幕组 → 查看剧集预览<br/>
          3. 选择下载模式：<b>自动下载</b>（新增剧集自动下载）或 <b>手动管理</b>（在下载管理中手动选择）<br/>
          4. 点击「订阅」即可完成
        </p>
        <h3 style={h3Style}>管理订阅</h3>
        <p style={pStyle}>
          在「订阅列表」中可查看所有已订阅的番剧。点击某张卡片可以展开详情，随时切换自动/手动下载模式。点击「刷新全部」立即拉取所有启用订阅的最新剧集。
        </p>
      </div>

      {/* 2. Download */}
      <div style={sectionStyle}>
        <h2 style={h2Style}>⬇ 下载管理</h2>
        <h3 style={h3Style}>自动下载</h3>
        <p style={pStyle}>
          订阅模式设为"自动下载"后，新剧集会立即进入 aria2c 下载队列。您可以在下载管理页面暂停、恢复或删除正在进行的下载任务。下载进度每 2 秒自动刷新。
        </p>
        <h3 style={h3Style}>手动管理</h3>
        <p style={pStyle}>
          订阅模式设为"手动管理"后，新剧集会进入<b>待处理</b>区域，按番剧分组展示。您可以：<br/>
          · 勾选需要的剧集（支持全选）后点击「下载选中」批量开始下载<br/>
          · 点击单集的「立即下载」单独开始<br/>
          · 同一话数如有多个版本（如简/繁体），行背景会高亮为黄色，方便对比选择
        </p>
        <h3 style={h3Style}>手动添加下载</h3>
        <p style={pStyle}>
          点击「添加下载」可以手动输入磁力链接或本地 .torrent 文件路径进行下载。
        </p>
        <h3 style={h3Style}>清理残留文件</h3>
        <p style={pStyle}>
          下载完成后会残留 .torrent 和 .aria2 中间文件。点击「清理残留文件」可以一键递归清理下载目录中的所有中间文件（不会影响已下载的视频），正在进行中的下载任务文件会自动跳过。
        </p>
      </div>

      {/* 3. Library */}
      <div style={sectionStyle}>
        <h2 style={h2Style}>📂 本地番剧</h2>
        <p style={pStyle}>
          本地番剧页面会自动扫描下载目录中的视频文件（支持 mkv、mp4、avi、mov、webm、flv、ts），按番剧分组展示。
        </p>
        <h3 style={h3Style}>操作说明</h3>
        <p style={pStyle}>
          · <b>展开/收起</b>：点击番剧标题行即可展开该组的所有剧集<br/>
          · <b>播放</b>：点击每集右侧的「播放」按钮使用系统默认播放器打开视频<br/>
          · <b>打开文件夹</b>：点击标题行右侧的「打开文件夹」在资源管理器中定位<br/>
          · <b>标记已看</b>：点击「标记已看」追踪观看进度，已看剧集会以浅绿背景显示
        </p>
      </div>

      {/* 4. Settings */}
      <div style={sectionStyle}>
        <h2 style={h2Style}>⚙ 设置</h2>
        <h3 style={h3Style}>下载目录</h3>
        <p style={pStyle}>
          默认下载目录为项目根目录下的 <span style={codeStyle}>download/</span> 文件夹。可自定义为任意路径。RSS 订阅的剧集将按字幕组名称在子文件夹内存放。
        </p>
        <h3 style={h3Style}>RSS 刷新间隔（分钟）</h3>
        <p style={pStyle}>
          后台自动拉取 RSS 更新的时间间隔，默认 30 分钟。最小 5 分钟，最大 1440 分钟（24 小时）。修改后即时生效，无需重启。
        </p>
        <h3 style={h3Style}>aria2 端口</h3>
        <p style={pStyle}>
          aria2c JSON-RPC 通信端口，默认 6800。仅当端口冲突时需要修改。
        </p>
        <h3 style={h3Style}>最大同时下载数</h3>
        <p style={pStyle}>
          aria2c 同时进行的下载任务上限，默认 5 个。
        </p>
        <h3 style={h3Style}>下载后自动删除种子文件</h3>
        <p style={pStyle}>
          开启后，下载完成时自动删除对应的 .torrent 文件。
        </p>
        <h3 style={h3Style}>关闭窗口最小化到系统托盘</h3>
        <p style={pStyle}>
          默认开启。点击窗口关闭按钮时程序不会退出，而是最小化到系统托盘图标。可通过托盘菜单「打开窗口」恢复或「退出」彻底关闭程序。
        </p>
      </div>

      {/* Version */}
      <div style={{ textAlign: "center", fontSize: 11, color: "var(--text-muted)", padding: "16px 0" }}>
        bangumiao v0.2.x · 基于 Tauri 2.x + React 构建
      </div>
    </div>
  );
}
