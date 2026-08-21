# Cat Character Test / 猫猫猫格大测试

[中文](#中文) | [English](#english)

## 中文

一款纯前端猫咪性格测试。主人根据猫咪最近 30 天的日常动作完成 32 道观察题，应用从四个原创维度生成 16 种猫格收藏卡：

- 亲近节奏：社牛 ↔ 慢热
- 领地偏好：探险 ↔ 安稳
- 关系距离：贴贴 ↔ 独处
- 刺激反应：细腻 ↔ 松弛

首版形象能力：

- 12 种统一复古印刷风格的内置猫咪头像，其中第 03 位是蓝金渐层形象。
- 支持上传 PNG、JPEG 或 WebP，自家猫照片只在浏览器 Canvas 中裁剪、色阶化和抖动处理，不会上传或写入持久化存储。
- 支持一次选择或持续添加多张自家猫照片，并可逐张删除；删除当前头像时会自动选中下一张，全部删除后才回到内置形象库。
- 完成测试后也可直接更换结果卡形象：既能重新选择 12 个内置猫猫，也能上传新照片；切换不会重新计算猫格，可用系统截图保存当前结果。
- 重新打开应用时仅恢复内置头像选择；上传的照片不会跨会话保留。

每个维度包含 4 道正向题和 4 道反向题。单轴恰好打平时，维度条会显示“均衡”；16 型归类使用该轴最后一道非中性回答作平局判定，若整轴均为中性则归入低刺激一侧。

### 隐私与权限

- 不联网、不执行命令，不启用 Node、AI 或 Agent，也不申请文件写入权限。
- 仅使用 `app.storage` 在本机保存答题进度和最近 5 次结果。
- 头像处理与结果卡生成都在浏览器 Canvas 内完成，不上传任何内容。
- 首版不提供文件下载；结果页提示使用系统截图保存当前猫格卡。
- 结果仅供娱乐和日常观察，不构成兽医或行为学诊断。

### 目录

```text
cat-personality-test/
├── meta.json
├── package.json
├── storage.json
├── source/
│   ├── index.html
│   ├── style.css
│   ├── ui.js
│   ├── worker.js
│   ├── esm_dependencies.json
│   └── assets/              # 可重新嵌入源码的压缩 WebP 资源
└── scripts/
    ├── embed-cat-assets.mjs
    ├── build-market-package.mjs
    └── preview-server.mjs
```

如需从压缩资源重新生成源码中的内嵌图片，可运行 `npm run assets:embed`。

### 本地预览

```bash
node scripts/preview-server.mjs
```

打开 `http://127.0.0.1:4178/?locale=zh-CN&theme=light`。可将 `locale` 改为 `en-US`，将 `theme` 改为 `dark`。

### 安装到 BitFun

将目录复制到 BitFun 用户数据目录的 `miniapps/cat-personality-test/` 下，然后重启或刷新 MiniApp 目录。市场包只包含 `meta.json` 和 `source/` 下的六个必需文本文件。

## English

A pure-frontend BitFun MiniApp. The owner answers 32 questions about observable behavior during the past 30 days. The app produces one of 16 original profiles across four dimensions and renders it as a collectible card. It includes twelve illustrated portraits and can style an uploaded cat photo locally with Canvas.

The app requests no network, filesystem, shell, Node, AI, or Agent access. It uses `app.storage` only for local progress and the five most recent results. Uploaded photos are processed entirely in browser Canvas, and the result portrait can be changed without recalculating the profile. The first release asks users to save the visible result with a system screenshot. Results are for reflection and fun, not medical or behavioral diagnosis.
