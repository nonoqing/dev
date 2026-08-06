# Remote Connect 用户指南索引

> 用途：索引产品当前直接链接的 Remote Connect 配置指南。
> 范围：`docs/remote-connect/`。
> 状态：temporary in-repo boundary。
> 权威语言：中文指南为中文，英文指南为英文。

本目录只暂存用户配置指南，不放运行时架构、内部部署 runbook、凭据、密钥或新的通用用户手册。

Web UI 的 [`RemoteConnectDialog.tsx`](../../src/web-ui/src/app/components/RemoteConnectDialog/RemoteConnectDialog.tsx)
当前把以下文件的仓库 URL 作为产品外链。因此不能只移动或删除文档；迁移必须在
同一次修改中提供稳定公开 URL、更新代码引用与聚焦测试，并验证旧产品入口不再依赖仓库路径。

| Guide | Language |
|---|---|
| [`feishu-bot-setup.zh-CN.md`](feishu-bot-setup.zh-CN.md) | 中文 |
| [`feishu-bot-setup.md`](feishu-bot-setup.md) | English |
