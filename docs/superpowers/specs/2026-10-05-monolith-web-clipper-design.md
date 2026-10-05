# Monolith 网页剪藏（Web Clipper）— 设计

> 日期：2026-10-05 · 状态：**已批准**  
> 前置：[`2026-10-04-local-document-asset-design.md`](./2026-10-04-local-document-asset-design.md)  
> 原则：**快速简单，套壳**——fork Obsidian Web Clipper，换品牌与保存出口；Monolith **不**自动监视 Inbox，由用户手动打开 MD 后登记。

---

## 0. 一句话

**Fork MIT 的 Obsidian Web Clipper → 换名换图标 → 剪藏 MD（+ sidecar）落到约定 inbox → 用户用 Monolith 手动打开并登记资产；大陆用 App 内置扩展目录 + 开发者模式加载（不依赖 Chrome 网上应用店）。**

---

## 1. 目标与非目标

### 1.1 目标（一期）

1. 浏览器扩展：当前页 → Markdown（复用上游抽取/模板能力）。
2. 保存到约定目录：`~/Downloads/MonolithInbox/`（冲突时 Chrome uniquify；sidecar 跟最终 MD 文件名对齐）。
3. 扩展写入伴生 `{stem}.monolith-clip.json`（`source` URL）+ MD frontmatter `source`；**App 不 watch、不弹导入提示**。
4. 用户打开 MD / 资产登记时：惰性写 `conversion_log`（`input_path`=页面 URL，`output_path`=MD，`tool=monolith-clipper`）+ `asset_create`（**不搬文件** L1）；`asset_instance.source_path` = **页面 URL**（非本地 html 路径）。
5. 查看源头 URL → **系统默认浏览器**打开。
6. App 内「安装网页剪藏」入口：释放/打开随包扩展目录 + 简短加载步骤（开发者模式）。

### 1.2 非目标（一期）

- Chrome Web Store / 静默注入 Chrome。
- Native Messaging、localhost 通知。
- Inbox 目录监视与自动导入提示。
- Crawl4AI / Docker / 本机重型抓取。
- 使用 Obsidian 商标、官方图标或营销素材。
- 自动搬家到「摘要库」；Ctrl+S 建版本等既有铁律不变。

### 1.3 二期（可选，不阻塞一期）

- 本机通道直写 App Support + 自动登记。
- 抽取失败时的增强抓取（Crawl4AI 等）。
- Edge 应用商店分发（若可达）。

---

## 2. 铁律

| # | 铁律 |
|---|------|
| W1 | 扩展源码基于 [obsidianmd/obsidian-clipper](https://github.com/obsidianmd/obsidian-clipper) **MIT**；保留许可证与版权声明。 |
| W2 | **禁止**使用 Obsidian 商标/图标/营销文案；产品名 **Monolith Clipper**（或等价自有名）。归因说明可写「Based on Obsidian Web Clipper (MIT)」。 |
| W3 | 一期保存出口 = 用户下载目录下的 **`MonolithInbox/`**（Chrome 扩展无法直写 Application Support）。 |
| W4 | Monolith **不** watch Inbox、**不**弹导入提示；与扩展进程解耦（无 NM）。用户手动打开 MD 后登记。 |
| W5 | 导入遵守资产 L1/L7：不删不搬源 MD；删登记不删文件。 |
| W6 | 大陆主路径 = **内置扩展目录 + 加载已解压扩展**；商店链接非必须。 |
| W7 | App 离线时可剪藏：扩展只落盘；打开 MD 时惰性补写 `conversion_log`。 |
| W8 | 源头为 http(s) 时，用系统默认浏览器打开（非 Finder）。 |

---

## 3. 组件与路径

```text
Chrome（Monolith Clipper 扩展）
        │  chrome.downloads → MonolithInbox/*.md + *.monolith-clip.json
        ▼
~/Downloads/MonolithInbox/
        │  用户手动用 Monolith 打开 .md
        ▼
Monolith App ──打开/登记──► conversion_log（惰性）+ asset_create(source_path=url)
```

| 路径 | 用途 |
|------|------|
| `~/Downloads/MonolithInbox/` | 剪藏落盘（一期约定；设置里可后置改） |
| `~/Library/Application Support/com.muqiang.monolith/MonolithClipper/`（由 App 从随包资源释放） | 供 Chrome「加载已解压」；与二进制/设置同属 App Support |
| `assets.sqlite` | 打开 MD / 登记资产时写台账 |

---

## 4. 扩展（套壳）

1. Fork upstream；改 `name` / `description` / 图标 / 包 ID。
2. 将「写入 Obsidian 库 / obsidian://」改为：**下载 MD 到 `MonolithInbox/`**（`chrome.downloads`，`filename: MonolithInbox/...`）。
3. Frontmatter 至少含：`source` 或 `url`；另写 `{stem}.monolith-clip.json`（与最终 uniquify 后的 MD 文件名对齐）。
4. 随 Monolith release **打包同一份扩展目录**（或 zip），供安装入口释放。

---

## 5. Monolith App

### 5.1 安装入口

- 设置或菜单：**安装网页剪藏扩展**
- 行为：确保扩展已释放到 `~/Library/Application Support/com.muqiang.monolith/MonolithClipper/` → Finder 打开该目录 → 尽量打开 `chrome://extensions` → 展示 3 步说明（开发者模式 → 加载已解压 → 选该目录）。
- App 升级覆盖扩展目录后，提示用户在扩展页点「重新加载」。
- 说明：Chrome 可能对未发布扩展提示停用——属平台限制，文案告知即可。

### 5.2 Inbox 与离线 log（无自动提示）

- 扩展写入 `~/Downloads/MonolithInbox/{name}.md` + 伴生 `{name}.monolith-clip.json`（`source` URL）；App **不** watch、**不**弹导入提示。
- **Monolith 离线时可剪藏**：扩展无法写 SQLite；打开 MD 时 `conversion_latest_source` **惰性补写** `conversion_log`（`input_path`=URL，`tool=monolith-clipper`），来源优先 sidecar，其次 frontmatter。
- 资产登记时若源为 http(s) URL，同样确保有 log；`source_path` = URL。

### 5.3 i18n

- 安装说明走现有 `zh` / `en`（`navigator.language`）。

---

## 6. DoD（一期）

1. 扩展可加载；剪藏文件出现在 `~/Downloads/MonolithInbox/`（含 sidecar）。  
2. 用户手动打开 MD 并资产登记后，`source_path` / `conversion_log.input_path` 为页面 URL；「查看源头」打开系统浏览器。  
3. App 内安装入口能打开目录 + 说明（无需商店）。  
4. 无 Obsidian 品牌资源；MIT 声明保留。  
5. 不引入 Crawl4AI / Native Messaging / Inbox watch。

---

## 7. 实现顺序（套壳）

1. Fork 扩展 → 改品牌 → 改保存到 `MonolithInbox` + sidecar。  
2. Monolith：释放扩展目录 + 安装说明 UI。  
3. Monolith：打开 MD / 登记时惰性 `conversion_log` + URL 浏览器打开。  
4. 手工验收：文章页剪一页 → 用 Monolith 打开 → 查看源头 → 登记资产。
