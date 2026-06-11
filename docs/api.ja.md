# Sona HTTP API Reference

Sona は、外部ツールやヘッドレス環境から利用するための、プライバシー重視のローカル HTTP API サーバーを提供します。自動化ツール、バッチ処理スクリプト、補助アプリから、安全な REST API を通じてローカルの speech-to-text ワークフローを操作できます。

---

## 設定とサーバー起動

API Server は 2 通りの方法で起動できます。

### 1. GUI Client Settings

`Settings > API Server` を開き、次の項目を設定します。

- **Enable API Server**: API サーバーを有効または無効にします。
- **Host**: バインドする IP アドレスです。`127.0.0.1` はローカルマシンに限定し、`0.0.0.0` はすべてのネットワークインターフェースで待ち受けます。
- **Port**: サーバーの TCP ポートです。既定値は `14200` です。
- **API Key**: エンドポイントを保護する任意の Bearer トークンです。`Generate` で安全なキーを作成し、`Copy` でクリップボードへコピーできます。

### 2. Headless CLI Mode

ターミナルから、デスクトップ UI なしで API サーバーだけを起動できます。

```bash
sona serve --host 127.0.0.1 --port 14200 --api-key your_secure_key --ip-whitelist localhost --max-streaming 2 --gpu-acceleration auto
```

GPU アクセラレーションは、GUI のモデル設定または `sona serve --gpu-acceleration` によって、サーバーレベルの既定値として設定します。バッチ API とストリーミング API の各リクエストでは、GPU 設定を個別に上書きできません。

---

## Authentication

**API Key** が設定されている場合、すべての HTTP リクエストに Bearer トークンとして `Authorization` ヘッダーを含める必要があります。

```http
Authorization: Bearer your_secure_key
```

Settings で API Key が空の場合、サーバーは認証なしのリクエストを許可します。

---

### 1. Server Info & Capabilities

サーバーのプラットフォーム情報、ハードウェア状態、インストール済みモデル、利用可能なオンライン ASR プロバイダーを取得します。

- **URL**: `/v1/info`
- **Method**: `GET`

#### Response (`200 OK`)

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

### 2. Server Health & Stats

サーバーの稼働時間、アクティブ / 待機中ジョブ数、一時ストレージ使用量を取得します。

- **URL**: `/health`
- **Method**: `GET`

#### Response (`200 OK`)

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

### 3. List All Jobs

現在ジョブマネージャーが保持しているすべての文字起こしジョブ状態を取得します。

- **URL**: `/v1/transcriptions/jobs`
- **Method**: `GET`

#### Response (`200 OK`)

`job_id` から現在の `JobStatus` への map が返ります。

```json
{
  "c86e0c65-2746-4e56-9141-866d51bbca43": "Pending",
  "a1b2c3d4-e5f6-4g7h-8i9j-k0l1m2n3o4p5": "Processing"
}
```

---

### 4. Submit Transcription Job

ローカルの音声または動画ファイルを送信して、speech-to-text 処理を実行します。ジョブはキューに入り、バックグラウンドの文字起こし worker によって順番に処理されます。

- **URL**: `/v1/transcriptions`
- **Method**: `POST`
- **Content-Type**: `multipart/form-data`

#### Request Payload (Multipart Form Fields)

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `file` | Binary | **Yes** | 文字起こし対象の音声または動画ファイル。 |
| `model_id` | String | **Yes** | ローカル ASR モデル ID（例: `sensevoice`）、または設定済み Cloud ASR プロバイダー ID（例: `volcengine-doubao`）。 |
| `language` | String | No | 対象言語コード（例: `zh`, `en`, `ja`, `ko`, `yue`）。既定値は `"auto"`。 |
| `hotwords` | String | No | 認識精度を高めるためのカスタム語彙 / キーワード。改行区切りで指定します。 |
| `webhook_url` | String | No | 文字起こしが完了または失敗したときに POST 通知を受け取る HTTP URL。 |
| `webhook_secret` | String | No | Webhook ペイロードを HMAC-SHA256 で署名するための secret。 |

#### Response (`200 OK`)

この文字起こしタスクに割り当てられた一意の `job_id` を返します。

```json
{
  "job_id": "c86e0c65-2746-4e56-9141-866d51bbca43"
}
```

#### Curl Example

```bash
curl -X POST http://127.0.0.1:14200/v1/transcriptions \
  -H "Authorization: Bearer your_secure_key" \
  -F "file=@/path/to/interview.wav" \
  -F "model_id=sensevoice" \
  -F "language=zh"
```

---

### 5. Query Job Status

送信済みジョブの現在のライフサイクル状態と文字起こし結果を取得します。

- **URL**: `/v1/transcriptions/:job_id`
- **Method**: `GET`

#### Response Structs

ジョブの進行状態に応じて、次のいずれかの JSON パターンが返ります。

##### A. Pending

ジョブはキュー内で文字起こし worker の空きを待っています。

```json
"Pending"
```

##### B. Processing

ジョブはアクティブで、現在文字起こし中です。

```json
"Processing"
```

##### C. Completed

文字起こしが成功しました。ミリ秒単位の timestamp を含むセグメントテキストを返します。

```json
{
  "Completed": [
    {
      "id": 0,
      "start": 120,
      "end": 2840,
      "text": "Hello, welcome to Sona.",
      "speaker": "Speaker 0"
    },
    {
      "id": 1,
      "start": 3100,
      "end": 5600,
      "text": "We are processing speech locally on your machine.",
      "speaker": "Speaker 1"
    }
  ]
}
```

##### D. Failed

文字起こしが失敗しました。具体的なエラーメッセージを含みます。

```json
{
  "Failed": "Failed to decode audio file: invalid format"
}
```

#### Curl Example

```bash
curl http://127.0.0.1:14200/v1/transcriptions/c86e0c65-2746-4e56-9141-866d51bbca43 \
  -H "Authorization: Bearer your_secure_key"
```

---

## Webhooks & Verification

ジョブ作成時に `webhook_url` を指定した場合、Sona はジョブ完了または失敗時に、その URL へ最終 JSON 状態を POST します。

### Webhook Signature (`X-Sona-Signature`)

Webhook を保護するには、ジョブ送信時に `webhook_secret` を指定します。Sona はこの secret を使って JSON ペイロード文字列の HMAC-SHA256 signature を計算し、次のヘッダーで送信します。

- **Header Name**: `X-Sona-Signature`
- **Format**: `sha256=<hex_encoded_signature>`

#### Verification Algorithm (Node.js Example)

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
