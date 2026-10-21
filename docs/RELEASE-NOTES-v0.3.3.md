# Monolith v0.3.3

在 **v0.3.2** 之上：Pi Agent SDK 更新到 GitHub `earendil-works/pi` **main** 最新 tip（npm 官方包仍停在 1.1.0，故改为构建时从源码 vendor）。

## 变更

- **Pi Agent**：`npm run pi-agent:install` 默认克隆/构建 `main`，打包进 Bridge（记录 commit SHA 于 `bridge/package.json` → `piSource`）。
- 可选：`PI_FROM_NPM=1` 仍可用 registry；`PI_REF=<tag>` 可钉版本。
- 继承 v0.3.2：内置 Node、端口可改/自避让、CORS。

## 构建

```bash
npm run pi-node:install
npm run pi-agent:install
npx tauri build
```
