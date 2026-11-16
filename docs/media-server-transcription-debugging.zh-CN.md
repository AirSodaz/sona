# 媒体服务器字幕漏段排查

桌面日志使用 JSON 行记录，保留现有的时间、级别、目标、文件和行号字段，并增加 schema 版本、毫秒时间戳、来源、组件、事件和结构化上下文。客户端控制台与文件使用同一条脱敏记录；错误对象保留名称、消息、堆栈和 cause。URL、认证信息、令牌、配置、音频、字幕文本和 payload 字段会被隐藏，日志不记录完整音频或转录文本。

使用包含诊断日志的构建，在设置的通用页面把日志级别调为 `debug`，重新提交一个容易复现的影片。记录字幕缺失的起止时间，以及在原片中该处是否有对白。任务结束后，从设置的关于页面打开日志文件夹，保留此次运行对应的 `appsona` 日志。排查结束后可调回 `info`。

新提交的任务会记录服务器报告的影片时长 `expectedDurationSeconds`；旧队列恢复的任务可能没有此字段。时长元数据只用于对照，不会触发重试或阻止字幕上传。

## 日志约定与审计范围

- 前端生产代码统一使用 logger；Biome 禁止直接调用 console。旧的消息调用方式保持可用，带组件前缀的消息会生成 component，形如 task_start 的事件名会生成 event，其余消息的 event 为 message。
- 桌面文件继续沿用 appsona 名称、10 MB 大小和 5 份轮转；运行时日志级别设置不变。info 记录任务起止和每 60 秒音频读取进度，debug 记录逐分段时间范围，warn/error 记录丢弃、回退、中断和失败。
- 客户端记录以 source=frontend 标识，结构化上下文位于 context；宿主保留 level、time、timestamp_ms、target、file、line 的所有权。后端记录以 source=backend 标识，诊断事件和 instance_id 位于 message 中。
- API 服务补齐排队、拒绝、开始、完成、失败、删除和关停日志；本地识别使用 job_id 作为诊断 instance_id。桌面在线 ASR 补齐起止和失败记录，指标也带上 instance_id。
- CLI 继续将诊断输出到 stderr，并复用脱敏规则；MCP 的 stdout 协议输出保持原样。已检查 Android 客户端的静态初始化日志，本次未发现需要修改的问题。
- webhook、在线 ASR endpoint、Google 翻译和 FFmpeg 错误输出使用脱敏摘要。字符串中的 HTTP/WebSocket URL 会整体替换，结构化敏感字段会隐藏；不要主动把原文、音频或完整请求对象传入日志。

## 关联一次任务

1. 找到 `[BatchQueue] task_start`，确认 `itemId`、`audioStreamIndex` 和预计时长。
2. 用同一 `taskId` 找到 `asr_instance_assigned`，取得 `instanceId`。
3. 用 `instance_id` 筛选 `[BatchASR]` 和 `[BatchStream]`。兼容的音轨下载路径可通过 `audio_download_complete.downloadFile` 关联 `[MediaAudio]`。
4. 对照 `asr_complete` 和 `subtitle_sync_start` 的分段统计，检查处理后是否减少内容。`subtitle_sync_complete` 表示同步调用成功返回；不代表字幕覆盖完整。

## 如何解释日志

| 证据 | 下一步检查 |
| --- | --- |
| `read_end.received_s` 明显小于影片时长 | 检查网络/媒体响应和 `decoder_end`、`decoder_stderr`；正常 EOF 也不保证原片已全部读完。 |
| `decoder_end` 非正常退出或出现 `read_interrupted` | 根据已脱敏的错误信息检查 FFmpeg、媒体格式和网络。 |
| 漏段处没有 `slice_start`，但实际读取时长覆盖到此处 | 检查 VAD 设置和原片音轨；没有分段并不能单独证明该处有语音。 |
| 漏段处有 `slice_skipped` | 查看 `reason`，区分无结果、空识别、标点处理后为空、识别线程异常。 |
| 有 `generation_limit` | llama.cpp 该段生成达到了 token/上下文预算，可能提前截断。 |
| ASR 后覆盖正常，字幕同步前分段数/空段数发生变化 | 检查项目流水线、润色、文本替换等后处理。 |
| 音轨下载中断或字节数异常 | 检查 `[MediaAudio] download_response`、`download_interrupted` 和 `download_complete` 的字节数。无响应长度时，字节数不能证明完整。 |

`largestTimelineGap` 只是字幕时间轴中最大空隙，片头、静音和无对白镜头也会产生空隙。不要据此自动补字幕。

流式 FFmpeg 当前使用默认音频映射，`decoder_end` 会标注 `audio_mapping=default`。队列记录的 `audioStreamIndex` 是请求选择，不能证明服务器直传视频中的该音轨已被 FFmpeg 选中；多音轨影片需核对实际声音。

这些日志能缩小到读取、分段、识别或后处理阶段。若 ASR 成功返回非空文字但漏掉片段内的对白，仍需对照相应音频复现，日志无法判断文字内容是否正确。新增日志不记录完整字幕或音频，FFmpeg 诊断中的 URL 会被脱敏。
