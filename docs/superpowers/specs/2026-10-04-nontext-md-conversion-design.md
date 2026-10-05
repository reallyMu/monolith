# 非文本 → Markdown 转换与履历（Monolith）— 设计

> 日期：2026-10-04 · 状态：**已批准 / 修订（downmark）**  
> 前置：[`2026-10-04-local-document-asset-design.md`](./2026-10-04-local-document-asset-design.md)  
> 栈：Tauri 2 + SQLite + Vue 3；转换器：**downmark**（~8MB 单二进制）

---

## 0. 一句话

非文本经打包的 **downmark** 转成 MD → 写 **conversion_log** → 打开 MD；**仅资产登记**时把 log 最新源写入 `source_path`。转换中全应用模态阻塞，可取消。保持小工具体积（App + 转换器约十余 MB，不捆绑 minerU/markitdown）。

---

## 1. 目标与非目标

### 1.1 目标

1. 统一转换引擎：`downmark`（PDF 文本层 / Office / HTML / CSV 等，见 `convert-types.json`）。
2. `conversion_log` 追加；查询键 = 输出 MD 绝对路径；登记回填取最新成功。
3. 输出路径可选；默认源同目录同名 `.md`。
4. 入口：打开/拖入可转类型；菜单「转换为 Markdown…」。
5. 转换中模态阻塞 + 可取消。
6. 资产右键：Finder 显示源文件（有 `source_path`）。

### 1.2 非目标

- minerU / markitdown / 其它重型 Python+模型栈随包交付。
- 扫描件 OCR、图片转 MD。
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
| C4 | **唯一工具** `downmark`；扩展名白名单见 `convert-types.json`。 |
| C5 | 二进制在 `third-party/downmark/`（~8MB），随 App Resources 交付；缺失则失败提示安装脚本。 |
| C6 | 转换模态阻塞；取消杀进程并记 `cancelled`。 |

---

## 3. `conversion_log` / 源快照

同库 `assets.sqlite`。

`conversion_log`：`id, input_path, output_path, tool, status, started_at, finished_at, message,` **`input_mtime`, `input_size`**（Unix 秒 / 字节；转换时快照）。

`asset_instance` 溯源扩展：**`source_path`, `source_mtime`, `source_size`**（登记或成功重转时快照）。

**陈旧判定**：源文件仍存在，且当前 `mtime` 或 `size` 与快照不一致 → `stale`，提示重新转换（可覆盖原 MD 输出路径）。

---

## 4. 打包

- `third-party/downmark/downmark` + `run`（`run <in> <out.md>` → `downmark -o out in`）
- 刷新：`scripts/install-converters.sh`
- 不提交巨型 venv；二进制可用脚本拉取 release

---

## 5. DoD

1. 可转类型一律 downmark；未知/图片扩展名拒绝。  
2. 成功/失败/取消写 log；登记取最新 success。  
3. 模态 + 取消。  
4. App 体积保持小工具量级（转换器 ~8MB 级）。  
5. 资产 L1–L9 回归通过。

---

## 6. 候选工具备忘（未接入）

记录于 2026-10-04。**当前生产仍是 downmark**；此节只存档，不改 C4。

产品口径：**随包约百 MB 可接受**（JRE + 引擎合计约 70–100MB 这一档可以；数 GB 的 minerU/模型栈仍不随包）。

### OpenDataLoader PDF

| | |
|---|---|
| 上游 | https://github.com/opendataloader-project/opendataloader-pdf · https://opendataloader.org |
| 角色 | 本地 PDF → JSON / Markdown / HTML；默认 CPU、无 GPU |
| 体积 | CLI / PyPI wheel **~22MB**（v2.5.12：`opendataloader-pdf-cli-2.5.12.zip` 21.5MB，wheel 21.6MB） |
| 运行时 | **Java 11+**（JRE 约 50–80MB，不随 zip）。合计大约 **70–100MB** |
| Hybrid / OCR | `opendataloader-pdf[hybrid]` + Docling 等后端 → **GB 级**，不随包 |
| 许可 | Apache-2.0 |
| 对照 minerU | 官方基准默认模式总分接近 minerU（~0.83），CPU 更快；hybrid 更高但不纳入体积预算 |
| 接入注意 | 引擎是 Java CLI，不是单文件 native；要随包需打 JRE 或要求本机 JDK。主要覆盖 **PDF**，Office/HTML/CSV 仍可能要 downmark 或其它 |

**不选 hybrid。** 若升级转换器，优先评估：默认 CLI zip + 捆绑/探测 JRE，PDF 走 OpenDataLoader，其它类型仍 downmark。
