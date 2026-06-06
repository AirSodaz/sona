#!/usr/bin/env python3
"""
Sona API Streaming Transcription Example
Streams a local 16kHz mono 16-bit PCM WAV file via WebSockets to the Sona API.

Requires: pip install websockets
"""

import argparse
import asyncio
import json
import os
import sys
import wave
import websockets

async def receive_messages(ws):
    try:
        async for message in ws:
            data = json.loads(message)
            msg_type = data.get("type")
            if msg_type == "started":
                print(f"Session started! Session ID: {data.get('session_id')}")
            elif msg_type == "segment":
                seg = data.get("segment", {})
                prefix = "[Final]" if seg.get("is_final") else "[Temp]"
                speaker = seg.get("speaker") or "Unknown"
                print(f"{prefix} [{seg.get('start'):.2f}s - {seg.get('end'):.2f}s] ({speaker}): {seg.get('text')}")
            elif msg_type == "stopped":
                print("Session stopped by server.")
                break
            elif msg_type == "error":
                print(f"Server Error: {data.get('message')}")
                break
    except websockets.exceptions.ConnectionClosed:
        print("WebSocket connection closed by server.")
    except Exception as e:
        print(f"Error receiving messages: {e}")

async def stream_audio(uri, wave_path, model_id, language, hotwords, token):
    if not os.path.exists(wave_path):
        print(f"Error: File not found at {wave_path}")
        sys.exit(1)

    # Check WAV properties
    try:
        with wave.open(wave_path, 'rb') as wf:
            channels = wf.getnchannels()
            sampwidth = wf.getsampwidth()
            framerate = wf.getframerate()
            if channels != 1 or sampwidth != 2 or framerate != 16000:
                print(f"Error: WAV file must be 16kHz, mono, 16-bit PCM. Found: {framerate}Hz, {channels}ch, {sampwidth*8}bit")
                sys.exit(1)

            # Build query params
            ws_uri = f"{uri}?token={token}" if token else uri
            print(f"Connecting to Sona WebSocket at {uri}...")

            async with websockets.connect(ws_uri, open_timeout=10) as ws:
                # 1. Send Start payload
                start_msg = {
                    "type": "start",
                    "model_id": model_id,
                    "language": language
                }
                if hotwords:
                    hotwords_list = [w.strip() for w in hotwords.split(",") if w.strip()]
                    if hotwords_list:
                        start_msg["hotwords"] = "\n".join(hotwords_list)

                await ws.send(json.dumps(start_msg))

                # Start background message reader
                receiver_task = asyncio.create_task(receive_messages(ws))

                print("Streaming audio chunks...")
                # 16000 samples/sec * 2 bytes/sample * 0.1 seconds = 3200 bytes per chunk (100ms)
                # Send 1600 frames (3200 bytes) every 100ms
                chunk_size = 1600

                while True:
                    data = wf.readframes(chunk_size)
                    if not data:
                        break
                    await ws.send(data)
                    await asyncio.sleep(0.1) # Simulate real-time stream ingestion

                print("Audio stream finished. Sending Stop message...")
                stop_msg = {"type": "stop"}
                await ws.send(json.dumps(stop_msg))

                # Wait for the server to send the Stopped message and close connection
                await receiver_task
    except Exception as e:
        print(f"Streaming failed: {e}")
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="Sona WebSocket Streaming Client")
    parser.add_argument("--file", required=True, help="Path to 16kHz mono 16-bit PCM WAV file")
    parser.add_argument("--model-id", default="sensevoice", help="ASR Model ID (must support streaming)")
    parser.add_argument("--language", default="auto", help="Language code (e.g., zh, en, auto)")
    parser.add_argument("--hotwords", default="", help="Optional hotwords for transcription (comma-separated)")
    parser.add_argument("--api-key", default="", help="Optional Sona API Key")
    parser.add_argument("--host", default="127.0.0.1", help="API server host")
    parser.add_argument("--port", type=int, default=14200, help="API server port")
    args = parser.parse_args()

    uri = f"ws://{args.host}:{args.port}/v1/streaming"
    try:
        asyncio.run(stream_audio(uri, args.file, args.model_id, args.language, args.hotwords, args.api_key))
    except KeyboardInterrupt:
        print("\nStreaming interrupted by user.")
        sys.exit(1)
    except Exception as e:
        print(f"Streaming failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
