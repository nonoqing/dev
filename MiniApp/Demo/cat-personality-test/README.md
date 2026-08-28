# Cat Character Test / 猫猫猫格大测试

[中文](#中文) | [English](#english)

## 中文

一款纯前端猫咪性格测试。主人根据猫咪最近 30 天的日常动作完成 32 道观察题，应用从四个原创维度生成 16 种猫格收藏卡：

- 亲近节奏：主动 ↔ 观察
- 探索偏好：尝鲜 ↔ 守成
- 行动风格：专注 ↔ 灵活
- 刺激反应：细腻 ↔ 松弛

首版形象能力：

- 12 种统一复古印刷风格的内置猫咪头像，其中 02 金白、05 浅金渐层、09 银虎斑来自真实猫猫照片；第 03 位蓝金渐层形象保持不变。
- 支持上传 PNG、JPEG 或 WebP，自家猫照片只在浏览器 Canvas 中裁剪、色阶化和抖动处理，不会上传或写入持久化存储。
- 支持一次选择或持续添加多张自家猫照片，并可逐张删除；删除当前头像时会自动选中下一张，全部删除后才回到内置形象库。
- 开始答题时会锁定当时选中的猫猫形象，答完不会自行变化；完成测试后也可重新选择 12 个内置猫猫或上传新照片，切换不会重新计算猫格。
- 下载严格以结果页当前所见为准：中文页面导出中文、英文页面导出英文，刚换好的头像会进入完整的 1600 × 900 PNG；文件名包含猫格称号与生成时间，避免误开旧卡片。
- 重新打开应用时仅恢复内置头像选择；上传的照片不会跨会话保留。

每个维度包含 4 道正向题和 4 道反向题。单轴恰好打平时，维度条会显示“均衡”；16 型归类使用该轴最后一道非中性回答作平局判定，若整轴均为中性则归入低刺激一侧。

v3 将原先与「亲近节奏」语义重叠的「关系距离」替换为「行动风格」，改为观察猫猫在玩耍、捕猎、解谜和受阻后的选择。升级后会保留猫猫名字与内置头像，但旧题库的未完成进度和历史结果不会套用到新维度，需重新作答。

### 隐私与权限

- 不联网、不执行命令，不启用 Node、AI 或 Agent，也不申请文件写入权限。
- 仅使用 `app.storage` 在本机保存答题进度和最近 5 次结果。
- 头像处理与结果卡生成都在浏览器 Canvas 内完成，不上传任何内容。
- 结果卡通过浏览器原生下载保存到系统默认下载目录，不需要申请文件系统权限；截图仍可作为备选方式。
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

A pure-frontend BitFun MiniApp. The owner answers 32 questions about observable behavior during the past 30 days. The app produces one of 16 original profiles across four cat-native dimensions: approach rhythm, exploration preference, action style, and stimulus response. It renders the result as a collectible card, includes twelve illustrated portraits, and can style an uploaded cat photo locally with Canvas.

Version 9 replaces the overlapping relationship-distance axis with action style, focusing on play, pursuit, problem solving, and responses to obstacles. Cat name and built-in portrait selection survive the upgrade, while incomplete answers and history from the previous question model are reset rather than reinterpreted.

The app requests no network, filesystem, shell, Node, AI, or Agent access. It uses `app.storage` only for local progress and the five most recent results. The portrait selected at quiz start is pinned through the result, while the result portrait can still be changed without recalculating the profile. The full 1600 × 900 PNG is rendered from the result page's currently visible language, text, and portrait, then downloaded with the same browser Blob flow as PPT Live. Screenshots remain an optional fallback. Results are for reflection and fun, not medical or behavioral diagnosis.
