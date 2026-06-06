#!/usr/bin/env node
// Requires: npm install ws
/**
 * Sona API Streaming Transcription Example
 * Streams a local 16kHz mono 16-bit PCM WAV file via WebSockets to the Sona API.
 */

import fs from 'fs';
import path from 'path';
import WebSocket from 'ws';

function main() {
  const args = process.argv.slice(2);
  let filePath = '';
  let modelId = '';
  let language = 'auto';
  let hotwords = '';
  let apiKey = '';
  let host = '127.0.0.1';
  let port = '14200';

  // Parse arguments (supports both --key value options and positional parameters)
  const hasOptions = args.some(arg => arg.startsWith('--'));

  if (hasOptions) {
    for (let i = 0; i < args.length; i++) {
      if (args[i] === '--file') {
        filePath = args[++i];
      } else if (args[i] === '--model-id') {
        modelId = args[++i];
      } else if (args[i] === '--language') {
        language = args[++i];
      } else if (args[i] === '--hotwords') {
        hotwords = args[++i];
      } else if (args[i] === '--api-key') {
        apiKey = args[++i];
      } else if (args[i] === '--host') {
        host = args[++i];
      } else if (args[i] === '--port') {
        port = args[++i];
      }
    }
  } else {
    if (args.length < 2) {
      console.log("Usage: node stream_transcribe.js <wav_file_path> <model_id> [language] [hotwords] [api_key] [host] [port]");
      console.log("   or: node stream_transcribe.js --file <wav_file_path> --model-id <model_id> [--language <language>] [--hotwords <hotwords>] [--api-key <api_key>] [--host <host>] [--port <port>]");
      process.exit(1);
    }
    filePath = args[0];
    modelId = args[1];
    language = args[2] || "auto";
    hotwords = args[3] || "";
    apiKey = args[4] || "";
    host = args[5] || "127.0.0.1";
    port = args[6] || "14200";
  }

  // Validate that modelId is not empty or undefined
  if (!modelId) {
    console.error("Error: Model ID is required. Please specify a model ID using --model-id <model_id> or as the second positional argument.");
    process.exit(1);
  }

  // Validate that the input file exists
  if (!filePath || !fs.existsSync(filePath)) {
    console.error(`Error: File not found at ${filePath}`);
    process.exit(1);
  }

  // Validate WAV properties (must be 16kHz, mono, 16-bit PCM)
  let fd;
  const header = Buffer.alloc(44);
  let readSuccess = false;
  try {
    fd = fs.openSync(filePath, 'r');
    fs.readSync(fd, header, 0, 44, 0);
    readSuccess = true;
  } catch (err) {
    console.error(`Error validating WAV file: ${err.message}`);
  } finally {
    if (fd !== undefined) {
      try {
        fs.closeSync(fd);
      } catch (closeErr) {
        // ignore
      }
    }
  }

  if (!readSuccess) {
    process.exit(1);
  }

  const riff = header.toString('ascii', 0, 4);
  const wave = header.toString('ascii', 8, 12);

  if (riff !== 'RIFF' || wave !== 'WAVE') {
    console.error("Error: WAV file is invalid (missing RIFF/WAVE header).");
    process.exit(1);
  }

  const audioFormat = header.readUInt16LE(20);
  const channels = header.readUInt16LE(22);
  const sampleRate = header.readUInt32LE(24);
  const bitsPerSample = header.readUInt16LE(34);

  if (audioFormat !== 1) {
    console.error(`Error: WAV file must be PCM. Found format code: ${audioFormat}`);
    process.exit(1);
  }
  if (channels !== 1) {
    console.error(`Error: WAV file must be mono (1 channel). Found: ${channels} channels`);
    process.exit(1);
  }
  if (sampleRate !== 16000) {
    console.error(`Error: WAV file sample rate must be 16000Hz. Found: ${sampleRate}Hz`);
    process.exit(1);
  }
  if (bitsPerSample !== 16) {
    console.error(`Error: WAV file bits per sample must be 16-bit. Found: ${bitsPerSample}-bit`);
    process.exit(1);
  }

  // Use the 'ws' library for WebSocket connections, passing 'token' as a query parameter if an API Key is provided
  const wsUri = apiKey ? `ws://${host}:${port}/v1/streaming?token=${apiKey}` : `ws://${host}:${port}/v1/streaming`;
  console.log(`Connecting to Sona WebSocket at ws://${host}:${port}/v1/streaming...`);

  const ws = new WebSocket(wsUri);
  let sessionStarted = false;

  ws.on('open', () => {
    console.log("Connected to Sona WebSocket server.");
    // 7. Send the Start JSON message (accepting 'model_id', 'language', and support optional 'hotwords' parsed from comma-separated CLI args and converted to newline-separated)
    const startMsg = {
      type: 'start',
      model_id: modelId,
      language: language
    };
    if (hotwords) {
      const hotwordsList = hotwords.split(',').map(w => w.trim()).filter(w => w.length > 0);
      if (hotwordsList.length > 0) {
        startMsg.hotwords = hotwordsList.join('\n');
      }
    }
    ws.send(JSON.stringify(startMsg));
  });

  // 8. Concurrently listen for server messages ('started', 'segment', 'stopped', 'error') and log formatted updates to the console
  ws.on('message', (data) => {
    try {
      const msg = JSON.parse(data.toString());
      if (msg.type === 'started') {
        console.log(`Session started! Session ID: ${msg.session_id}`);
        sessionStarted = true;
        // Begin streaming audio chunks
        startStreaming(ws, filePath);
      } else if (msg.type === 'segment') {
        const seg = msg.segment;
        const prefix = seg.is_final ? '[Final]' : '[Temp]';
        const speaker = seg.speaker || 'Unknown';
        const startTime = seg.start;
        const endTime = seg.end;
        const startStr = startTime !== undefined && startTime !== null ? `${startTime.toFixed(2)}s` : '0.00s';
        const endStr = endTime !== undefined && endTime !== null ? `${endTime.toFixed(2)}s` : '0.00s';
        console.log(`${prefix} [${startStr} - ${endStr}] (${speaker}): ${seg.text}`);
      } else if (msg.type === 'stopped') {
        console.log('Session stopped by server.');
        ws.close();
      } else if (msg.type === 'error') {
        const errMsg = msg.message || 'Unknown error';
        console.error(`Server Error: ${errMsg}`);
        process.exit(1);
      }
    } catch (err) {
      console.error('Failed to parse server message:', err);
      process.exit(1);
    }
  });

  ws.on('close', (code, reason) => {
    const reasonStr = reason.toString() || 'none';
    console.log(`WebSocket connection closed. Code: ${code}, Reason: ${reasonStr}`);
    if (code !== 1000) {
      process.exit(1);
    }
  });

  ws.on('error', (err) => {
    console.error('WebSocket connection error:', err);
    process.exit(1);
  });
}

function startStreaming(ws, wavePath) {
  // 4. Read the local WAV file using 'fs.createReadStream' with a chunk size of 3200 bytes ('highWaterMark: 3200') to match a 100ms interval for 16kHz 16-bit mono PCM.
  // 5. Skip the first 44 bytes of the WAV file to bypass the standard WAV header.
  const stream = fs.createReadStream(wavePath, { start: 44, highWaterMark: 3200 });

  console.log("Streaming audio chunks...");

  stream.on('data', (chunk) => {
    if (chunk.length > 0) {
      // 6. Simulate real-time streaming by pausing the file stream, sending the chunk, and resuming the stream after a 100ms delay ('setTimeout').
      stream.pause();
      ws.send(chunk, (err) => {
        if (err) {
          console.error('Error sending audio chunk:', err);
          process.exit(1);
        }
        setTimeout(() => {
          stream.resume();
        }, 100);
      });
    }
  });

  stream.on('error', (err) => {
    console.error('Audio stream error:', err);
    process.exit(1);
  });

  stream.on('end', () => {
    console.log("Audio file fully read. Sending Stop message...");
    // 7. Send the Stop JSON message at the end of the file.
    const stopMsg = { type: 'stop' };
    ws.send(JSON.stringify(stopMsg), (err) => {
      if (err) {
        console.error('Error sending stop message:', err);
        process.exit(1);
      }
    });
  });
}

main();
