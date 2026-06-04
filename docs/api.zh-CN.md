# Sona HTTP API 接口参考文档

Sona 提供了一个隐私优先的本地 HTTP API 服务，旨在用于外部无头（headless）集成或编程集成。自动化工具、批量处理脚本或辅助客户端能够通过一个安全的本地 REST API 调用完全本地化的离线语音转文字服务。

---

## 配置与服务启用

API 服务可以通过以下两种方式启动：

### 1. GUI 客户端设置
在主界面打开 **设置 -> API 服务** 并配置：
- **启用 API 服务**: 切换服务开启/关闭。
- **监听地址**: 绑定 IP（例如：`127.0.0.1` 仅限本机访问；`0.0.0.0` 绑定所有网络设备，允许局域网或公网外部访问）。
- **端口**: API 服务运行的 TCP 端口（默认：`14200`）。
- **安全密钥**: 用于保护接口的可选 Bearer 令牌。点击 **生成** 可以获得安全的随机密钥，点击 **复制** 写入剪贴板。

### 2. 命令行无头模式
您还可以直接通过命令行无头运行 Sona：
```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key --ip-whitelist localhost --max-streaming 2
```

---

## 接口认证

当配置了 **安全密钥**（API Key）后，每个发往 API 服务的请求都必须在 `Authorization` 请求头中包含 Bearer 令牌：

```http
Authorization: Bearer your_secure_key
```

如果设置中未配置 API Key，服务将允许任意未认证的请求。

---

### 1. 获取服务器信息与可用能力 (Server Info)

获取服务器平台信息、硬件状态、已安装模型以及可用的在线 ASR 提供商。

- **URL**: `/v1/info`
- **Method**: `GET`

#### 响应 (`200 OK`)

```json
{
  "platform": "win32",
  "gpuAvailable": true,
  "models": ["sensevoice", "sherpa-onnx-whisper-turbo"],
  "vadInstalled": true,
  "punctuationInstalled": true,
  "onlineAsrProviders": [
    {
      "id": "volcengine-doubao",
      "configured": true,
      "supportsBatch": true,
      "supportsStreaming": true
    }
  ]
}
```

---

### 2. 服务健康与统计 (Server Health)

获取服务器运行时间、活跃/排队任务数以及临时存储占用情况。

- **URL**: `/health`
- **Method**: `GET`

#### 响应 (`200 OK`)

```json
{
  "status": "ok",
  "uptime": 3600,
  "activeJobs": 1,
  "pendingJobs": 0,
  "cacheSpaceBytes": 10485760
}
```

---

### 3. 列出所有任务 (List All Jobs)

查询任务管理器中所有转录任务的当前状态。

- **URL**: `/v1/transcriptions/jobs`
- **Method**: `GET`

#### 响应 (`200 OK`)

返回 `job_id` 到其当前 `JobStatus` 的映射。

```json
{
  "c86e0c65-2746-4e56-9141-866d51bbca43": "Pending",
  "a1b2c3d4-e5f6-4g7h-8i9j-k0l1m2n3o4p5": "Processing"
}
```

---

### 4. 提交转录任务 (Submit Transcription Job)

提交本地音频或视频文件进行高性能的语音识别。转录工作流会被推入队列并由后台进程排队执行。

- **URL**: `/v1/transcriptions`
- **Method**: `POST`
- **Content-Type**: `multipart/form-data`

#### 请求参数 (Multipart 表单字段)

| 参数名称 | 类型 | 是否必填 | 参数说明 |
| :--- | :--- | :--- | :--- |
| `file` | 二进制 | **是** | 要转录的音频或视频文件。 |
| `model_id` | 字符串 | **是** | 本地已安装的 ASR 模型 ID（例如 `sensevoice`） 或已配置的云端 ASR 引擎 ID（例如 `volcengine-doubao`）。 |
| `language` | 字符串 | 否 | 目标识别语言（例如：`zh`、`en`、`ja`、`ko`、`yue`），默认为 `"auto"` 自动检测。 |
| `hotwords` | 字符串 | 否 | 自定义热词词汇表，用于提高特定术语识别率。每行一个热词。 |
| `webhook_url` | 字符串 | 否 | 当转录任务成功结束或失败时，用于接收结果 POST 推送的 URL。 |
| `webhook_secret` | 字符串 | 否 | 用于对 Webhook 推送报文进行 HMAC-SHA256 签名的密钥。 |

#### 返回数据 (`200 OK`)

返回包含为此转录任务分配的唯一 `job_id` 的 JSON 对象：

```json
{
  "job_id": "c86e0c65-2746-4e56-9141-866d51bbca43"
}
```

#### Curl 请求示例
```bash
curl -X POST http://127.0.0.1:14200/v1/transcriptions \
  -H "Authorization: Bearer your_secure_key" \
  -F "file=@/path/to/interview.wav" \
  -F "model_id=sensevoice" \
  -F "language=zh"
```

---

### 5. 查询任务状态 (Query Job Status)

查询转录任务的当前生命周期状态以及转录文本结果。

- **URL**: `/v1/transcriptions/:job_id`
- **Method**: `GET`

#### 状态返回数据结构

接口会根据任务的处理进度，返回以下 JSON 格式之一：

##### A. 排队中 (Pending)
任务目前在队列中排队，正等待空闲 worker 执行。
```json
"Pending"
```

##### B. 处理中 (Processing)
任务目前正在由识别引擎运行转录中。
```json
"Processing"
```

##### C. 成功完成 (Completed)
语音识别成功。返回包含毫秒级时间戳的分段转录文本列表：
```json
{
  "Completed": [
    {
      "id": 0,
      "start": 120,
      "end": 2840,
      "text": "你好，欢迎使用 Sona。",
      "speaker": "Speaker 0"
    },
    {
      "id": 1,
      "start": 3100,
      "end": 5600,
      "text": "我们正在您的本地机器上处理语音识别。",
      "speaker": "Speaker 1"
    }
  ]
}
```

##### D. 识别失败 (Failed)
转录任务由于异常中断或文件错误而失败。返回包含具体失败原因的 JSON 对象：
```json
{
  "Failed": "Failed to decode audio file: invalid format"
}
```

#### Curl 请求示例
```bash
curl http://127.0.0.1:14200/v1/transcriptions/c86e0c65-2746-4e56-9141-866d51bbca43 \
  -H "Authorization: Bearer your_secure_key"
```

---

## Webhooks 结果推送与安全校验

如果在提交任务时指定了 `webhook_url`，Sona 会在任务最终处于成功（Completed）或失败（Failed）状态时，向该 URL 发送包含最终 JSON 数据结构的 POST 请求。

### Webhook 签名计算 (`X-Sona-Signature`)

为了防止未经授权的冒充推送，可以在提交任务时提供一个 `webhook_secret`。Sona 将使用该密钥对 POST 请求的整个 body 字符串计算 HMAC-SHA256 签名，并通过请求头传递：

- **请求头名称**: `X-Sona-Signature`
- **数据结构**: `sha256=<hex_encoded_signature>`

#### 接收端校验算法示例 (Node.js)
```javascript
const crypto = require('crypto');

function verifySignature(payloadString, secret, receivedSignatureHeader) {
  const [algorithm, signature] = receivedSignatureHeader.split('=');
  if (algorithm !== 'sha256') return false;

  const expectedSignature = crypto
    .createHmac('sha256', secret)
    .update(payloadString)
    .digest('hex');

  return crypto.timingSafeEqual(
    Buffer.from(signature, 'hex'),
    Buffer.from(expectedSignature, 'hex')
  );
}
```
