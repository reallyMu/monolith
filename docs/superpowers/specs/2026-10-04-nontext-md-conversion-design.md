# 非文本 → Markdown 转换与履历（Monolith）— 设计

> 日期：2026-10-04 · 状态：**已批准 / 修订（OpenDataLoader PDF + downmark）**  
> 前置：[`2026-10-04-local-document-asset-design.md`](./2026-10-04-local-document-asset-design.md)  
> 栈：Tauri 2 + SQLite + Vue 3；**PDF → OpenDataLoader**；**图片 → RapidOCR**；其它可转类型 → **downmark**

---

## 0. 一句话

非文本经打包转换器转成 MD → 写 **conversion_log** → 打开 MD；**仅资产登记**时把 log 最新源写入 `source_path`。转换中全应用模态阻塞，可取消。PDF 用 OpenDataLoader；jpg/png/bmp/tiff 用 RapidOCR（捆绑 CPython + site-packages）；Office/HTML/CSV 等仍用 downmark。不捆绑 minerU / OpenDataLoader hybrid。

---

## 1. 目标与非目标

### 1.1 目标

1. 三引擎：`opendataloader-pdf`（PDF）+ `rapidocr`（jpg/png/bmp/tiff）+ `downmark`（其余，见 `convert-types.json`）。
2. `conversion_log` 追加；查询键 = 输出 MD 绝对路径；登记回填取最新成功。
3. 输出路径可选；默认源同目录同名 `.md`。
4. 入口：打开/拖入可转类型；菜单「转换为 Markdown…」。
5. 转换中模态阻塞 + 可取消。
6. 资产右键：Finder 显示源文件（有 `source_path`）。

### 1.2 非目标

- minerU / markitdown / OpenDataLoader **hybrid**。
- webp/gif 等未列入的图片格式；表格结构重建（OCR 仅按阅读序输出行文本）。
- 非文本直接进资产树。
- 未登记也算资产溯源。
- Job 队列 / 多任务并行。

---

## 2. 铁律

| # | 铁律 |
|---|------|
| C1 | 资产准入不变：仅 `file-types.json` 文本；产物默认 `.md`。 |
| C2 | 溯源只在资产：`source_path` 仅登记时写入。 |
| C3 | log **只 INSERT**，禁止 UPDATE/DELETE。含转换 success/failed/cancelled，以及索引重定位 `relocated`。回填：当前 `output_path` 取最新 `success`；若无则沿 `relocated` 链回溯。 |
| C4 | 工具由 `convert-types.json` 按扩展名路由；**pdf → opendataloader-pdf**，**jpg/jpeg/png/bmp/tif/tiff → rapidocr**，其余可转 → **downmark**。 |
| C5 | 引擎在 `third-party/downmark/`、`opendataloader-pdf/`、`rapidocr/`；运行时 `jre/`、`python/`；随 App Resources 交付；缺失则失败提示安装脚本。 |
| C6 | 转换模态阻塞；取消杀进程并记 `cancelled`。 |

---

## 3. `conversion_log` / 源快照

同库 `assets.sqlite`。

`conversion_log`：`id, input_path, output_path, tool, status, started_at, finished_at, message,` **`input_mtime`, `input_size`**（Unix 秒 / 字节；转换时快照）。

`asset_instance` 溯源扩展：**`source_path`, `source_mtime`, `source_size`**（登记或成功重转时快照）。

**陈旧判定**：源文件仍存在，且当前 `mtime` 或 `size` 与快照不一致 → `stale`，提示重新转换（可覆盖原 MD 输出路径）。

---

## 4. 打包

- `third-party/downmark/downmark` + `run`（`run <in> <out.md>`）
- `third-party/opendataloader-pdf/opendataloader-pdf-cli.jar` + `run`；App 侧直接 `java -jar … --markdown --no-json --image-output off -o <tmpdir>` 后拷到目标 `.md`
- `third-party/jre/`：Temurin 11（`scripts/install-jre.sh`）
- `third-party/python/` + `rapidocr/site-packages/`：`install-python.sh` + `install-converters.sh`
- 刷新：上述安装脚本；巨型运行时不进 git

---

## 5. DoD

1. PDF → OpenDataLoader；jpg/png/bmp/tiff → RapidOCR；其它可转 → downmark；未知扩展名拒绝。  
2. 成功/失败/取消写 log；登记取最新 success。  
3. 模态 + 取消。  
4. 不随包 hybrid / minerU。  
5. 资产 L1–L9 回归通过。

---

## 6. OpenDataLoader PDF（已接入）

| | |
|---|---|
| 上游 | https://github.com/opendataloader-project/opendataloader-pdf · https://opendataloader.org |
| 角色 | 本地 PDF → Markdown（亦支持 JSON/HTML；本产品只用 MD） |
| 体积 | CLI jar **~22MB** + Temurin 11 JRE **~100MB+** |
| Hybrid / OCR | **不随包** |
| 许可 | Apache-2.0 |

探测顺序：`MONOLITH_JAVA_HOME` → `JAVA_HOME` → 捆绑 `third-party/jre/bin/java` → PATH `java`。

### RapidOCR（已接入）

| | |
|---|---|
| 上游 | https://github.com/RapidAI/RapidOCR |
| 角色 | 本地图片 OCR → Markdown（按检测框阅读序逐行） |
| 体积 | site-packages ~300MB + 捆绑 CPython ~30–80MB |
| 入口 | `to_md.py` / `./run`；App 用捆绑 `python` + `PYTHONPATH=site-packages` |

探测顺序：`MONOLITH_PYTHON` → 捆绑 `third-party/python/bin/python3` → PATH `python3`。
