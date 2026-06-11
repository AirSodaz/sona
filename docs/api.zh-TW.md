# Sona HTTP API 介面參考文件

Sona 提供一個隱私優先的本機 HTTP API 服務，旨在用於外部無外介面 (headless) 整合或程式整合。自動化工具、批次處理指令碼或輔助用戶端能夠透過安全的本機 REST API 控制語音轉文字流程。

---

## 設定與服務啟用

API 服務可以透過以下兩種方式啟動：

### 1. GUI 應用程式設定
在主介面開啟 **設定 -> API 服務** 並設定：
- **啟用 API 服務**：切換服務開啟/關閉。
- **監聽地址**：繫結 IP（例如：`127.0.0.1` 僅限本機存取；`0.0.0.0` 繫結所有網路裝置，允許區域網或公網外部存取）。
- **連接埠**：API 服務執行的 TCP 連接埠（預設：`14200`）。
- **安全金鑰**：用於保護介面的可選 Bearer 權杖。點擊 **產生** 可以獲得安全的隨機金鑰，點擊 **複製** 寫入剪貼簿。

### 2. 命令列無外介面模式
您還可以直接透過命令列無外介面執行 Sona：
```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key --ip-whitelist localhost --max-streaming 2 --gpu-acceleration auto
```

GPU 硬體加速透過 GUI 模型設定或 `sona serve --gpu-acceleration` 作為服務級預設值設定。批次和流式 API 請求不支援依個別請求覆寫 GPU 設定。

---

## 介面認證

當設定了 **安全金鑰**（API Key）後，每個發往 API 服務的請求都必須在 `Authorization` 請求標頭中包含 Bearer 權杖：

```http
Authorization: Bearer your_secure_key
```

如果設定中未設定 API Key，服務將允許任意未認證的請求。

---

### 1. 獲取伺服器資訊與可用能力 (Server Info)

獲取伺服器平台資訊、硬體狀態、已安裝模型以及可用的線上 ASR 提供者。

- **URL**: `/v1/info`
- **Method**: `GET`

#### 回應 (`200 OK`)

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

### 2. 服務健康與統計 (Server Health)

獲取伺服器執行時間、活躍/佇列任務數以及臨時儲存佔用情況。

- **URL**: `/health`
- **Method**: `GET`

#### 回應 (`200 OK`)

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

### 3. 列出所有任務 (List All Jobs)

查詢任務管理器中所有轉錄任務的目前狀態。

- **URL**: `/v1/transcriptions/jobs`
- **Method**: `GET`

#### 回應 (`200 OK`)

傳回 `job_id` 到其目前 `JobStatus` 的對應。

```json
{
  "c86e0c65-2746-4e56-9141-866d51bbca43": "Pending",
  "a1b2c3d4-e5f6-4g7h-8i9j-k0l1m2n3o4p5": "Processing"
}
```

---

### 4. 提交轉錄任務 (Submit Transcription Job)

提交本機音訊或影片檔案進行高效能的語音辨識。轉錄工作流會進入佇列，並由背景程序依序執行。

- **URL**: `/v1/transcriptions`
- **Method**: `POST`
- **Content-Type**: `multipart/form-data`

#### 請求參數 (Multipart 表單欄位)

| 參數名稱 | 類型 | 是否必填 | 參數說明 |
| :--- | :--- | :--- | :--- |
| `file` | 二進位 | **是** | 要轉錄的音訊或影片檔案。 |
| `model_id` | 字串 | **是** | 本機已安裝的 ASR 模型 ID（例如 `sensevoice`） 或已設定的雲端 ASR provider ID（例如 `volcengine-doubao`）。 |
| `language` | 字串 | 否 | 目標辨識語言，例如 `zh`、`en`、`ja`、`ko` 或 `yue`。預設為 `auto`。 |
| `hotwords` | 字串 | 否 | 用於提升特定術語辨識率的自訂熱詞，每行一個。 |
| `webhook_url` | 字串 | 否 | 轉錄任務完成或失敗後接收 POST 通知的 URL。 |
| `webhook_secret` | 字串 | 否 | 用於對 Webhook payload 進行 HMAC-SHA256 簽名的金鑰。 |

#### 傳回資料 (`200 OK`)

傳回包含為此轉錄任務分配的唯一 `job_id` 的 JSON 物件：

```json
{
  "job_id": "c86e0c65-2746-4e56-9141-866d51bbca43"
}
```

#### Curl 請求範例
```bash
curl -X POST http://127.0.0.1:14200/v1/transcriptions \
  -H "Authorization: Bearer your_secure_key" \
  -F "file=@/path/to/interview.wav" \
  -F "model_id=sensevoice" \
  -F "language=zh"
```

---

### 5. 查詢任務狀態 (Query Job Status)

查詢轉錄任務的目前生命週期狀態以及轉錄文字結果。

- **URL**: `/v1/transcriptions/:job_id`
- **Method**: `GET`

#### 狀態傳回資料結構

介面會根據任務的處理進度，傳回以下 JSON 格式之一：

##### A. 佇列中 (Pending)
任務目前正在佇列中，等待空閒 worker 執行。
```json
"Pending"
```

##### B. 處理中 (Processing)
任務目前正在由辨識引擎執行轉錄中。
```json
"Processing"
```

##### C. 成功完成 (Completed)
語音辨識成功。傳回包含毫秒級時間戳記的分段轉錄文字清單：
```json
{
  "Completed": [
    {
      "id": 0,
      "start": 120,
      "end": 2840,
      "text": "你好，歡迎使用 Sona。",
      "speaker": "Speaker 0"
    },
    {
      "id": 1,
      "start": 3100,
      "end": 5600,
      "text": "我們正在您的本機上處理語音辨識。",
      "speaker": "Speaker 1"
    }
  ]
}
```

##### D. 辨識失敗 (Failed)
轉錄任務由於異常中斷或檔案錯誤而失敗。傳回包含具體失敗原因的 JSON 物件：
```json
{
  "Failed": "Failed to decode audio file: invalid format"
}
```

#### Curl 請求範例
```bash
curl http://127.0.0.1:14200/v1/transcriptions/c86e0c65-2746-4e56-9141-866d51bbca43 \
  -H "Authorization: Bearer your_secure_key"
```

---

## Webhooks 結果推送與安全驗證

如果在提交任務時指定了 `webhook_url`，Sona 會在任務最終處於成功（Completed）或失敗（Failed）狀態時，向該 URL 傳送包含最終 JSON 資料結構的 POST 請求。

### Webhook 簽名計算 (`X-Sona-Signature`)

為了防止未經授權的冒充推送，可以在提交任務時提供一個 `webhook_secret`。Sona 將使用該金鑰對 POST 請求的整個 body 字串計算 HMAC-SHA256 簽名，並透過請求標頭傳遞：

- **請求標頭名稱**: `X-Sona-Signature`
- **資料結構**: `sha256=<hex_encoded_signature>`

#### 接收端驗證演算法範例 (Node.js)
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
