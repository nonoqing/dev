# 贡献指南

[English](./CONTRIBUTING.md)

感谢关注 BitFun。BitFun 是 Rust + TypeScript 的多端 AI 编程环境，桌面端 / CLI / Server 共用一套核心逻辑。

本文面向**想参与贡献的人**：怎么搭环境、怎么提 PR、我们欢迎什么。  
**怎么改代码、架构边界、验证选型**以 [`AGENTS-CN.md`](AGENTS-CN.md) / [`AGENTS.md`](AGENTS.md) 为准——请勿把本文当成又一份命令清单或验证表。

## 行为准则

请保持尊重、友善、就事论事。欢迎不同背景与经验的贡献者。

## 快速开始

### 环境准备

- Node.js 22.12+（建议 LTS）
- pnpm 10.15.0（建议用 Corepack）
- Rust 工具链（[rustup](https://rustup.rs/)）
- 做桌面端还需 [Tauri 前置依赖](https://v2.tauri.app/start/prerequisites/)

本地 JS 构建与 CI 统一以 Node.js 22.12+ 为基线。仓库里的 GitHub Actions 可能用到兼容 Node.js 24 的 action runtime，但项目脚本仍默认按 22.12+ 编写。从更旧的 Node 切过来后，请重新执行 `pnpm install`。

#### 构建前置检查

当 `cargo check --workspace`、`cargo check -p bitfun-desktop` 或 pnpm 构建
命令报出难以理解的错误（如 "resource path doesn't exist" 或 sherpa-onnx
下载失败）时，运行前置检查以识别缺失的依赖并获取可操作的修复命令：

```bash
pnpm run check:build-prereqs           # 仅检查
pnpm run check:build-prereqs -- --fix  # 尝试自动修复缺失的前置依赖
```

检查项包括：

- 缺少 `node_modules`（修复：`pnpm install`）
- 缺少 `src/mobile-web/dist`（修复：`pnpm run prepare:mobile-web` —
  bitfun-desktop 的 Tauri 构建脚本将该目录作为资源引用，缺失时
  `cargo check -p bitfun-desktop` 和 `cargo check --workspace` 会失败）
- 缺少 sherpa-onnx 预编译库（sherpa-onnx-sys 构建脚本会在构建时从
  GitHub 下载；若网络连通性差导致下载失败，设置
  `SHERPA_ONNX_LIB_DIR` 指向 `target/sherpa-onnx-prebuilt/` 下的预编译
  lib 目录以使用本地副本）

### 安装依赖

```bash
pnpm install
```

### 日常命令

```bash
pnpm run desktop:dev                # 完整热更新：Vite HMR + Rust 自动重编并重启
pnpm run desktop:preview:debug      # 预构建二进制 + Vite HMR；Rust 不会自动重编
```

日常开发优先 `desktop:dev`；只迭代前端、或希望冷启动更快时用 `desktop:preview:debug`。

完整命令列表：[`docs/development/common-commands.zh-CN.md`](docs/development/common-commands.zh-CN.md)（亦可查 [`package.json`](package.json)）。
改完代码后，按 [`docs/development/verification.zh-CN.md`](docs/development/verification.zh-CN.md) 选**最小**检查。
架构与编码规范：[`AGENTS-CN.md`](AGENTS-CN.md)。

### 桌面端调试

dev 构建会打开 `devtools`：`F12` 打开原生 webview DevTools；`Cmd/Ctrl + Shift + I` 切换 BitFun 元素检查器；`Cmd/Ctrl + Shift + J` 也可打开原生 DevTools。面向用户的 `release` 包不带这些工具。

## 代码规范

架构、模块边界、i18n / 主题 / 日志、宿主与远程规则、验证矩阵，一律以 [`AGENTS-CN.md`](AGENTS-CN.md)（以及就近模块 `AGENTS.md`）为准。PR 对齐那些索引即可，不要把细则再抄进本文。

文档如何存放（Spec / Design / Plan）：[`docs/development/docs-governance.zh-CN.md`](docs/development/docs-governance.zh-CN.md)、[`docs/specs/README.md`](docs/specs/README.md)。

## 欢迎的贡献方向

1. 功能、交互、视觉等想法：提 Issue。产品 / UI 也可经 PI 快速投稿，我们会协助落到可开发状态。
2. 提升 Agent 系统与整体质量。
3. 稳定性与基础能力。
4. 扩展生态（Skills、MCP、LSP 插件，或特定领域场景支持）。

### 不止功能与修 bug

| 方向 | 位置 | 说明 |
| --- | --- | --- |
| Prompts | `src/crates/assembly/core/src/agentic/agents/prompts/` | 新增或优化提示词，并按需改相关逻辑 |
| Tools | `src/crates/assembly/core/src/agentic/tools/implementations/`、`.../registry.rs` | 实现工具并完成注册 |
| Subagents | `src/crates/assembly/core/src/agentic/agents/custom_subagents/`、`.../registry.rs` | 实现子代理并完成注册 |
| 模式 | `*_mode.rs`、`prompts/*_mode.md`、`src/web-ui/src/locales/*/settings/modes.json` | 模式逻辑、提示词与前端文案保持同步 |
| 场景指南 | `website/src/docs/` | 流程 / playbook（也可从 `README.md` 链过去） |

## 贡献流程与 PR

### 开始前

- 较大改动先开 Issue，减少重复劳动和设计打架。
- 新功能或 UI 变更尽早对齐设计方向。
- 按 Issue / PR 模板填写；PR 保持聚焦；若跳过某些验证，请在描述里说明原因。

### PR 标题与描述

建议 Conventional Commits：`feat:`、`fix:`、`docs:`、`chore:`、`refactor:`、`test:`。

UI 改动请附前后对比截图或短录屏。若借助了 AI，请注明，并说明测试深度（未测 / 轻测 / 已测）。

勿提交：临时 AI prompt、本机绝对路径、草稿产物、配对密钥、token、证书，以及其他无关文件。

### 分支与范围

**请向 `main` 提 PR。** 变更尽量小而专一，不要把无关改动捆在一起。

## 测试与验证

按改动文件选**最小**检查。完整矩阵见 [`docs/development/verification.zh-CN.md`](docs/development/verification.zh-CN.md)。宽范围套件交给 CI；只有影响构建 / 打包，或 CI 覆盖不到时，再在本地跑更重的命令。

跑不了相关检查时，在 PR 里说明原因，并给出风险更低的手动验证办法。

## 安全与合规

- 勿提交密钥、Token、证书或敏感信息。
- 新增依赖请确认许可证兼容，并简述用途。

## 感谢

Issue、PR 与建议都很重要，欢迎参与。
