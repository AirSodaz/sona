# Sona 用户指南

[English](user-guide.md) | [简体中文](user-guide.zh-CN.md) | [项目 README](../README.zh-CN.md)

这份指南面向桌面版终端用户，帮助您完成 Sona 的安装、首次配置、转录、编辑与导出。

本仓库中的配图目前使用的是参考示意图，而不是实时界面截图。它们已经使用稳定路径组织，后续如果替换成真实截图，不需要改动文档结构。

## 1. Sona 适合做什么

Sona 是一款以隐私优先为目标的本地转录编辑器，适合希望默认在本机完成语音转文字，而不是先把音频上传到云端的用户。

如果您有以下需求，Sona 会比较适合：

- 用 `Live Record` 实时录音并转录会议、课堂、访谈或语音笔记
- 用 `Batch Import` 批量转录已有的音频或视频文件
- 在按时间轴组织的编辑器中逐段修改文字
- 在自行配置 LLM 服务后，对文本进行润色或翻译
- 将结果导出为字幕或纯文本

## 2. 安装与启动

对大多数用户来说，最简单的方式是直接安装发布版本：

- 从 [GitHub Releases](https://github.com/AirSodaz/sona/releases/latest) 下载应用。
- 启动 Sona。
- 如果您需要从源码构建，请改看 [项目 README](../README.zh-CN.md)。

Sona 首次打开时，可能会直接显示 `First Run Setup`。如果之前跳过了配置，应用也可能持续显示提示横幅，直到离线模型配置完成为止。

## 3. 首次运行设置

在本地离线转录可用之前，Sona 需要先完成离线模型初始化。

### 前置条件

- Sona 可以正常启动。
- 如果您准备在首次引导中直接下载推荐模型包，需要联网。

### 操作步骤

1. 启动 Sona，等待 `First Run Setup` 出现。
2. 阅读欢迎步骤。Sona 推荐的首条成功路径是 `Microphone -> Live Record`。
3. 进入模型步骤，点击 `Download Recommended Models`。
4. 等待推荐离线模型包下载完成。
5. 进入麦克风步骤，并允许麦克风访问权限。
6. 选择您希望 `Live Record` 默认使用的麦克风。
7. 点击 `Start with Live Record`。

### 完成后会发生什么

- Sona 会应用推荐的本地离线转录配置。
- 应用会切换到 `Live Record`。
- 如果没有完成设置，后续也可以通过顶部提醒横幅重新打开引导。

### 说明

- 首次引导的目标是尽快让本地实时转录可以正常使用。
- 后续您可以在 `Settings > Model Hub` 中改用其他模型。
- 默认输入设备也可以在 `Settings > Input Device` 中重新调整。

## 4. 实时录音

如果您希望边录音边生成转录内容，请使用 `Live Record`。

### 前置条件

- 您已经完成 `First Run Setup`，或者手动配置了 `Live Record Model`。
- 如果要用麦克风录音，系统已经授予麦克风权限。

### 操作步骤

1. 点击顶部导航中的 `Live Record`。
2. 在未开始录音时，从下拉框中选择输入源：
   `Microphone` 或 `Desktop Audio`。
3. 点击红色的 `Start Recording` 按钮。
4. 通过波形和计时器观察采集状态。
5. 需要时使用 `Pause` 或 `Stop`。
6. 如果要调整识别参数，点击 `Parameter Settings`，设置：
   `Subtitle Mode`、`Language`、`Auto-Polish`、`Auto-Polish Frequency`。
7. 如果要启用实时字幕浮窗，打开 `Live Caption`，并在 `Settings > Subtitle Settings` 中调整浮窗行为。

### 完成后会发生什么

- 新的转录分段会实时出现在右侧编辑器中。
- 当前活跃分段会跟随播放和录音状态变化。
- 如果启用了 `Auto-Polish`，且 LLM 服务已配置完成，Sona 可以按批次自动润色已完成的分段。

### 说明

- `Ctrl + Space` 可以开始或停止实时录音。
- 录音过程中按 `Space` 可以暂停或恢复。
- 如果提示缺少模型，请重新完成 onboarding，或到 `Settings > Model Hub` 手动配置。

## 5. 批量导入

如果您已经有音频或视频文件，并希望在后台排队转录，请使用 `Batch Import`。

### 前置条件

- 您已经配置好了 `Batch Import Model`。
- 输入文件属于支持的音频或视频格式。

### 操作步骤

1. 点击 `Batch Import`。
2. 将文件拖入导入区域，或者点击 `Select File`。
3. 选择一个或多个文件。
4. 观察左侧队列和当前处理项的进度状态。
5. 如需继续追加文件，点击 `Add More Files`。
6. 如需调整转录参数，点击 `Parameter Settings`，设置 `Subtitle Mode`、`Language` 以及可选的 `Auto-Polish`。
7. 文件处理完成后，在右侧编辑器中检查转录内容。

### 完成后会发生什么

- Sona 会把文件加入队列，并显示 `Pending`、`Processing`、`Complete` 或 `Failed` 状态。
- 已完成的文件会载入主编辑器，供您继续编辑、翻译和导出。

### 说明

- 如果没有配置离线批量模型，Sona 会重新打开 onboarding，而不是直接开始导入。
- `Settings > Model Settings` 中的 `Max Concurrent Transcriptions` 与 `VAD Buffer Size` 会影响批处理体验。

## 6. 转录编辑与播放

当 Sona 生成转录分段之后，右侧编辑器就是主要工作区。

### 前置条件

- 您已经通过 `Live Record`、`Batch Import` 或 `History` 载入了转录内容。

### 操作步骤

1. 在编辑器中查看分段列表。
2. 点击时间戳可以跳转到对应播放位置。
3. 双击分段文本，或点击编辑按钮，进入编辑状态。
4. 按 `Enter` 保存当前分段。
5. 按 `Shift + Enter` 可在编辑时插入换行。
6. 使用合并按钮将当前分段与下一个分段合并。
7. 使用删除按钮在确认后删除分段。
8. 按 `Ctrl + F` 在转录内容中搜索。
9. 如果当前内容带有音频，使用底部播放器进行播放、暂停、跳转、倍速和音量控制。

### 完成后会发生什么

- 您可以逐段修正文本。
- 时间轴与播放位置会保持联动。

### 说明

- 只有在编辑某一段时，编辑工具栏才会显示。
- 工具栏支持 `Undo`、`Redo`、`Bold`、`Italic`、`Underline` 和换行。
- 搜索结果可以在不同分段之间快速跳转。

## 7. LLM 润色与翻译

Sona 的 LLM 功能是可选能力。离线转录本身不依赖它，但 `LLM Polish` 与 `Translate` 需要您先配置 LLM 服务。

### 前置条件

- 您已经有转录分段。
- 您已经在 `Settings > LLM Service` 中填写了：
  `LLM Service Type`、`Base URL`、`API Key`、`Model Name`，以及可选的 `Temperature`。

### LLM 润色步骤

1. 打开 `Settings > LLM Service`。
2. 选择服务提供方，例如 `OpenAI`、`Anthropic`、`Ollama`、`Google Gemini`、`DeepSeek`、`Kimi` 或 `SiliconFlow`。
3. 填入连接信息。
4. 点击 `Test Connection` 确认设置可用。
5. 回到主界面，点击 `LLM Polish` 按钮。
6. 根据当前状态，从菜单中选择开始润色、重新转录、撤销润色或重做润色。

### 翻译步骤

1. 确保当前转录内容已载入编辑器。
2. 点击 `Translate` 按钮。
3. 在菜单中选择目标语言。
4. 点击 `Start Translation` 或 `Retranslate`。
5. 使用 `Show Translations` 或 `Hide Translations` 控制是否在编辑器中显示双语内容。

### 完成后会发生什么

- `LLM Polish` 会直接更新转录文本。
- `Translate` 会将翻译结果保存到每个分段，并可显示在原文下方。

### 说明

- 当前支持的翻译目标语言包括 `Chinese (Simplified)`、`English`、`Japanese`、`Korean`、`French`、`German` 和 `Spanish`。
- `Parameter Settings` 中的 `Auto-Polish` 依赖有效的 LLM 配置。

## 8. 导出字幕与文本

只要编辑器里至少有一个分段，就可以导出内容。

### 前置条件

- 当前编辑器中已经有转录内容。

### 操作步骤

1. 点击顶部栏中的 `Export`。
2. 选择导出模式：
   `Original`、`Translation` 或 `Bilingual`。
3. 选择导出格式：
   `SubRip (.srt)`、`WebVTT (.vtt)`、`JSON (.json)` 或 `Plain Text (.txt)`。
4. 保存导出文件。

### 完成后会发生什么

- Sona 会按您选择的格式导出文件。
- 如果已有翻译内容，您可以导出仅翻译文本或双语版本。

### 说明

- 只有在至少一个分段包含翻译文本时，`Translation` 和 `Bilingual` 模式才可用。
- `Original` 模式始终可用。

## 9. 历史记录与常用设置

`History` 用于重新打开之前的工作，`Settings` 用于管理默认行为。

### History 使用步骤

1. 点击 `History`。
2. 按标题或转录内容搜索。
3. 使用 `All Types`、`Recordings`、`Batch Imports` 按类型筛选。
4. 使用 `Any Time`、`Today`、`Last 7 Days`、`Last 30 Days` 按时间筛选。
5. 点击某个条目即可重新载入。
6. 如果要批量删除，先进入选择模式。

### 建议优先了解的设置项

- `Settings > General`
  主题、界面语言、字体、托盘行为、自动检查更新
- `Settings > Input Device`
  麦克风选择、系统音频设备选择、麦克风增益、录音时静音
- `Settings > Subtitle Settings`
  实时字幕启动行为、点击穿透锁定、置顶、字体大小、宽度、颜色
- `Settings > Model Hub`
  `Live Record Model`、`Batch Import Model`、识别模型、标点模型、VAD 模型
- `Settings > Model Settings`
  `VAD Buffer Size`、`Max Concurrent Transcriptions`、ITN 设置、恢复默认值
- `Settings > Shortcuts`
  播放、实时录音、搜索与编辑快捷键
- `Settings > About`
  源码地址与更新检查

## 10. 常见问题与排障

### Sona 一直提示我完成首次设置

- 打开顶部提示横幅，把模型和麦克风步骤完成。
- 如果您之前跳过了引导，请确认 `Settings > Model Hub` 中 `Live Record Model` 与 `Batch Import Model` 都已配置。

### `Live Record` 无法开始

- 检查系统麦克风权限。
- 确认实时转录模型已经配置。
- 检查当前输入源是否选错，必要时在 `Microphone` 和 `Desktop Audio` 之间切换。

### `Batch Import` 无法开始

- 确认 `Batch Import Model` 已配置。
- 确认文件扩展名属于支持格式。
- 如果 Sona 提示格式不支持，请先把文件转成支持的音频或视频格式再导入。

### `LLM Polish` 或 `Translate` 不可用，或者执行失败

- 确认 `Settings > LLM Service` 中的 provider、base URL、API key 与 model 都填写正确。
- 先用 `Test Connection` 验证连通性。
- 如果您使用自定义接口地址或本地服务，例如 `Ollama`，请先确认那个服务本身可访问。

### 导出菜单里只有 `Original`

- 只有当当前转录至少包含一个翻译分段时，`Translation` 和 `Bilingual` 才会出现。

### 看不到播放控件

- 只有当当前转录关联了音频 URL 时，底部播放器才会显示。比如从历史记录载入的录音，或已处理完成的文件。

### 我想从源码构建或参与开发

- 请查看 [项目 README](../README.zh-CN.md) 中的源码构建与开发命令说明。
