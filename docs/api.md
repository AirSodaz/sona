# Sona HTTP API Reference

Sona exposes a privacy-first, local HTTP API server designed for external headless integration. Automated tools, batch-processing scripts, or secondary apps can control speech-to-text workflows locally through a secure REST API.

---

## Configuration & Server Activation

The API Server can be started in two ways:

### 1. GUI Client Settings
Navigate to **Settings -> API Server** (or **API 服务** in Chinese) and configure:
- **Enable API Server**: Toggle activation.
- **Host**: Bind IP address (e.g., `127.0.0.1` restricts access to the local machine; `0.0.0.0` binds to all network interfaces for LAN/WAN access).
- **Port**: TCP port for the server (default: `14200`).
- **API Key**: Optional Bearer token to protect endpoints. Click **Generate** to create a secure key, and **Copy** to write it to your clipboard.

### 2. Standalone CLI
The same API server adapter is available from the standalone CLI:

```bash
sona-cli serve --host 127.0.0.1 --port 14200 --api-key your_secure_key
```

The CLI server supports local REST transcription with installed offline models. The `serve` routes do not expose Online ASR or WebSocket streaming; use `sona-cli transcribe` or `sona-cli transcribe-live` for direct online transcription.

---

## Authentication

When an **API Key** is configured, every HTTP request must include it in the `Authorization` header as a Bearer token:

```http
Authorization: Bearer your_secure_key
```

If no API Key is set in Settings, the server permits unauthenticated requests.

---

### 1. Server Info & Capabilities

Retrieve server platform information, hardware status, installed models, and available online ASR providers.

- **URL**: `/info`
- **Method**: `GET`

#### Response (`200 OK`)

```json
{
  "platform": "win32",
  "gpuAvailable": true,
  "models": [
    {
      "id": "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
      "name": "SenseVoice (Int8)",
      "languages": ["en", "ja", "ko", "yue", "zh"],
      "languageMode": "auto"
    }
  ],
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

Retrieve server uptime, active/pending job counts, and temporary storage usage.

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

Query the current status of all transcription jobs in the manager.

- **URL**: `/v1/transcriptions/jobs`
- **Method**: `GET`

#### Query Parameters

| Parameter | Type | Required | Default | Description |
| :--- | :--- | :--- | :--- | :--- |
| `status` | String | No | - | Filter jobs by status: `pending`, `processing`, `completed`, `failed`. Case-insensitive. |
| `limit` | Integer | No | - | Maximum number of jobs to return. |
| `offset` | Integer | No | `0` | Number of jobs to skip before collecting results. |
| `full` | Boolean | No | `false` | When `false`, completed jobs return empty segment arrays `[]` to conserve bandwidth. Set `true` to return full completed segments. |

#### Response (`200 OK`)

Returns an ordered map (chronological by task creation time) of `job_id` to their current `JobStatus`.
```json
{
  "c86e0c65-2746-4e56-9141-866d51bbca43": "Pending",
  "a1b2c3d4-e5f6-4g7h-8i9j-k0l1m2n3o4p5": "Processing"
}
```

---

### 4. Submit Transcription Job

Submit a local audio or video file for high-performance speech-to-text processing. Jobs are queued and executed sequentially by the background transcription worker.

- **URL**: `/v1/transcriptions`
- **Method**: `POST`
- **Content-Type**: `multipart/form-data`

#### Request Payload (Multipart Form Fields)

| Field Name | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `file` | Binary | **Yes** | The audio or video file to be transcribed. |
| `model_id` | String | **Yes** | The identifier of a local ASR model (e.g., `sensevoice`) OR a configured Cloud ASR provider (e.g., `volcengine-doubao`). |
| `language` | String | No | Target language code (ISO 639-1, e.g. `zh`, `en`, `ja`). Defaults to `"auto"` for automatic detection. Only codes the target model actually supports take effect; unsupported values are coerced back to `"auto"`. Each model's supported list is exposed via `GET /info` (`onlineAsrProviders[].languages` / `languageMode`) and the model catalog. |
| `hotwords` | String | No | Custom vocabulary/keywords to enhance recognition, separated by newlines. |
| `webhook_url` | String | No | HTTP URL to receive a POST notification once the transcription is finished or fails. |
| `webhook_secret` | String | No | Secret key used to sign the webhook payload using HMAC-SHA256. |

#### Response (`200 OK`)

Returns a JSON object containing the unique `job_id` allocated for this transcription task:

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

Query the current lifecycle state and transcription results of a submitted job.

- **URL**: `/v1/transcriptions/:job_id`
- **Method**: `GET`

#### Response Structs

Depending on the job's progress, the endpoint returns one of the following JSON patterns:

##### A. Pending
The job is in queue waiting for the transcription worker.
```json
"Pending"
```

##### B. Processing
The job is active and transcription is currently underway.
```json
"Processing"
```

##### C. Completed
The transcription was successful. Returns segment-level text with millisecond timestamps:
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
The transcription failed. Includes the specific error message:
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

### 6. Delete / Cancel Job

Cancel an ongoing transcription or remove a completed/failed job and its associated audio file.

- **URL**: `/v1/transcriptions/:job_id`
- **Method**: `DELETE`

#### Response (`204 No Content`)

The job and temporary audio file are removed immediately, and any ongoing processing task is aborted.

#### Curl Example
```bash
curl -X DELETE http://127.0.0.1:14200/v1/transcriptions/c86e0c65-2746-4e56-9141-866d51bbca43 \
  -H "Authorization: Bearer your_secure_key"
```

---

### 7. Retrieve Job Audio

Stream or download the original audio file submitted with the job. Supports HTTP Range headers for seekable media player playback.

- **URL**: `/v1/transcriptions/:job_id/audio`
- **Method**: `GET`

#### Query Parameters

| Parameter | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `token` | String | No | Alternative to Bearer token header for HTML5 `<audio>` / `<video>` tags. |

#### Response (`200 OK` or `206 Partial Content`)

Returns binary audio stream with appropriate `Content-Type` (e.g. `audio/wav`, `audio/mpeg`).

---

### 8. Export Job Subtitles

Export completed transcription segments formatted as subtitles or formatted text.

- **URL**: `/v1/transcriptions/:job_id/export`
- **Method**: `GET`

#### Query Parameters

| Parameter | Type | Required | Default | Description |
| :--- | :--- | :--- | :--- | :--- |
| `format` | String | No | `srt` | Subtitle/export format: `srt`, `vtt`, `json`, `txt`, `md`. |
| `mode` | String | No | `original` | Text mode: `original` (source transcription), `translation` (translated text only), `bilingual` (source followed by translation). |

#### Response (`200 OK`)

Includes `Content-Disposition: attachment; filename="{job_id}.{format}"` and corresponding content type.

---

### 9. LLM Segment Polish

Polish raw speech segments using the configured desktop LLM provider.

- **URL**: `/v1/llm/polish`
- **Method**: `POST`
- **Content-Type**: `application/json`

#### Request Body

```json
{
  "segments": [
    {
      "id": "seg-0",
      "start": 0.0,
      "end": 2.5,
      "text": "uh hello world",
      "isFinal": true
    }
  ]
}
```

#### Response (`200 OK`)

```json
{
  "segments": [
    {
      "id": "seg-0",
      "start": 0.0,
      "end": 2.5,
      "text": "Hello, world.",
      "isFinal": true
    }
  ]
}
```

---

### 10. LLM Segment Translation

Translate transcription segments to a target language.

- **URL**: `/v1/llm/translate`
- **Method**: `POST`
- **Content-Type**: `application/json`

#### Request Body

```json
{
  "segments": [
    {
      "id": "seg-0",
      "start": 0.0,
      "end": 2.5,
      "text": "Hello, world.",
      "isFinal": true
    }
  ],
  "target_language": "zh"
}
```

#### Response (`200 OK`)

Returns segments with the `translation` field populated.

---

### 11. WebSocket Real-Time Audio Streaming

Real-time speech recognition stream via WebSocket.

- **URL**: `/v1/streaming` (or `ws://127.0.0.1:14200/v1/streaming?token=your_secure_key`)
- **Protocol**: WebSocket

#### Handshake Authentication
Pass API key in query (`?token=...` or `?api_key=...`) or `Authorization: Bearer <key>` header.

#### Protocol Flow
1. **Client -> Server (Text JSON)**:
   ```json
   {
     "type": "start",
     "model_id": "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
     "language": "auto",
     "hotwords": null,
     "vad_model_id": "silero-vad"
   }
   ```
2. **Server -> Client (Text JSON)**:
   ```json
   {
     "type": "started",
     "session_id": "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d"
   }
   ```
3. **Client -> Server (Binary)**:
   Raw PCM 16kHz 16-bit little-endian mono audio chunks (`pcm_s16le`).
4. **Server -> Client (Text JSON)**:
   Incremental recognized segments:
   ```json
   {
     "type": "segment",
     "segment": {
       "id": "...",
       "text": "Hello world",
       "start": 0.0,
       "end": 1.8,
       "isFinal": false
     }
   }
   ```
5. **Client -> Server (Text JSON)**:
   `{"type": "stop"}` to finalize and close stream.
---

## Webhooks & Verification

If `webhook_url` was specified during job submission, Sona posts the final JSON state to that URL on job completion or failure.

### Webhook Signature (`X-Sona-Signature`)

To secure your webhooks, specify a `webhook_secret` when submitting the job. Sona will compute an HMAC-SHA256 signature of the JSON payload string using this secret and send it in the headers:

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
