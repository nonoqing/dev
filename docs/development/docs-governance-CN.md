# 本仓文档规范

用途：约定 BitFun **代码仓**里文档如何存放、如何撰写、如何索引。  
适用范围：仓库内 `docs/`、根目录 `AGENTS` / `CONTRIBUTING`、以及模块旁的 `AGENTS.md`。  
状态：stable（目标结构已定；`ade-spec` / `features` / `plans` / `superpowers` 已并入 `specs/`）  
权威语言：中文（本文件）。英文摘要见 [`docs-governance.md`](docs-governance.md)。

独立 docs 仓（用户手册、接入指南等）不在本文范围内。

## 分仓

| 放代码仓 | 放独立 docs 仓 |
|---|---|
| 改本仓代码时必读的边界与操作约定 | 用户手册、接入手册、对外宣传说明 |
| 架构约束、验证矩阵、命令列表 | 培训材料、营销长文、与实现弱相关的长篇 |
| 随 PR 推进的规格与实施计划（进行中或已稳定） | 纯历史归档、运维搭建指南 |
| 模块旁 `AGENTS.md` / `LOGGING.md` | — |

优先迁出：`docs/remote-connect/`。

## 本仓 `docs/` 结构

```text
docs/
  architecture/     # 稳定架构；ADR 也放这里（不另建顶层 ADR 目录）
  development/      # 开发操作：命令、验证、宿主/远程、agent-loop、本文
  specs/            # 规格 + 计划（索引见 README）
    README.md
    templates/
    plans/
  sdlc-harness/     # 目标项目治理产品能力（跟实现相关，留本仓）
```

`ade-spec/`、`features/`、`plans/`、`superpowers/` 已并入 `specs/`（旧目录已删除）。

## 二级索引

```text
AGENTS.md  →  目录 README / 单篇权威文档  →  （最多再跳一次）正文
```

- 到正文最多两跳。
- 文章集合会变动的目录（`specs/`、`architecture/`、`sdlc-harness/`）必须有 `README` 索引。
- 高频单篇可由 AGENTS 直接链接（如 `product-architecture.md`、`verification.md`）。

## 语言

| 类型 | 语言 | 是否双语 |
|---|---|---|
| 面向人阅读的说明、流程 | 以中文为准 | 默认不强制英文 |
| 根目录 `AGENTS` / `CONTRIBUTING` | — | 中英都要有，语义必须对齐 |
| 主要给 AI / 改代码时查阅的操作与约束（如 `development/*`、模块 `AGENTS`） | 以英文为准 | 默认不强制中文副本 |
| 日志 | 只用英文 | 不做中文或双语日志 |

## 格式

- 页首写清：用途、适用范围、状态（draft/stable）、权威语言、相关链接。
- 能链到权威文档就不要把正文再抄一份。
- 文件名用英文 kebab-case。

## Spec / Design / Plan

- 流程与索引：[`docs/specs/README.md`](../specs/README.md)
- 空模板：[`docs/specs/templates/`](../specs/templates/)
- 跨模块计划：[`docs/specs/plans/`](../specs/plans/)

## 根入口

| 文件 | 位置 | 职责 |
|---|---|---|
| `AGENTS.md` / `AGENTS-CN.md` | 仓库根 | 改代码规范入口；渐进披露，细则外链 |
| `CONTRIBUTING.md` / `CONTRIBUTING_CN.md` | 仓库根 | 人如何参与；命令/验证链到 `development/*`，规范链到 AGENTS |

二者互相链接；CONTRIBUTING 不再维护第三套完整命令清单。

## 相关

- 命令：[`common-commands-CN.md`](common-commands-CN.md)
- 验证：[`verification-CN.md`](verification-CN.md)
- 规范入口：[`AGENTS.md`](../../AGENTS.md) / [`AGENTS-CN.md`](../../AGENTS-CN.md)
- 贡献指南：[`CONTRIBUTING.md`](../../CONTRIBUTING.md) / [`CONTRIBUTING_CN.md`](../../CONTRIBUTING_CN.md)
