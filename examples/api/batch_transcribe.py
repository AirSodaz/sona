#!/usr/bin/env python3
"""
Sona API Batch Transcription Example
Submits a local audio/video file to Sona API, polls the status, and prints segments.
"""

import argparse
import os
import sys
import time

import requests

def main():
    parser = argparse.ArgumentParser(description="Sona Batch Transcription Client")
    parser.add_argument("--file", required=True, help="Path to local audio/video file")
    parser.add_argument("--model-id", default="sensevoice", help="ASR Model ID or Cloud Provider ID")
    parser.add_argument("--language", default="auto", help="Language code (e.g., zh, en, auto)")
    parser.add_argument("--api-key", default="", help="Optional Sona API Key")
    parser.add_argument("--host", default="127.0.0.1", help="API server host")
    parser.add_argument("--port", type=int, default=14200, help="API server port")
    args = parser.parse_args()

    if not os.path.exists(args.file):
        print(f"Error: File not found at {args.file}")
        sys.exit(1)

    base_url = f"http://{args.host}:{args.port}"
    headers = {}
    if args.api_key:
        headers["Authorization"] = f"Bearer {args.api_key}"

    # 1. Submit Transcription Job
    upload_url = f"{base_url}/v1/transcriptions"
    print(f"Uploading {args.file} to Sona API...")

    try:
        with open(args.file, "rb") as f:
            files = {"file": f}
            data = {
                "model_id": args.model_id,
                "language": args.language
            }
            response = requests.post(upload_url, headers=headers, files=files, data=data, timeout=30)
    except requests.RequestException as e:
        print(f"Failed to submit transcription job: {e}")
        sys.exit(1)

    if response.status_code != 200:
        print(f"Error submitting job (HTTP {response.status_code}): {response.text}")
        sys.exit(1)

    job_info = response.json()
    job_id = job_info.get("job_id")
    if not job_id:
        print("Error: job_id is missing or empty in the response")
        sys.exit(1)
    print(f"Job successfully submitted! Job ID: {job_id}")

    # 2. Poll Job Status
    status_url = f"{base_url}/v1/transcriptions/{job_id}"
    print("Polling job status...")

    poll_interval = 2.0
    while True:
        try:
            response = requests.get(status_url, headers=headers, timeout=30)
        except requests.RequestException as e:
            print(f"Polling error: {e}")
            sys.exit(1)

        if response.status_code != 200:
            print(f"Error querying job status (HTTP {response.status_code}): {response.text}")
            sys.exit(1)

        status_data = response.json()

        # The status response will be a string (Pending/Processing) or a dict (Completed/Failed)
        if status_data == "Pending":
            print("Status: Pending (in queue)...")
        elif status_data == "Processing":
            print("Status: Processing...")
        elif isinstance(status_data, dict):
            if "Completed" in status_data:
                segments = status_data["Completed"]
                print("\nTranscription Completed Successfully!")
                print("=" * 60)
                for seg in segments:
                    start_time = seg.get("start", 0.0)
                    end_time = seg.get("end", 0.0)
                    text = seg.get("text", "")
                    speaker = seg.get("speaker") or "Unknown"
                    print(f"[{start_time:.2f}s - {end_time:.2f}s] ({speaker}): {text}")
                print("=" * 60)
                break
            elif "Failed" in status_data:
                print(f"Status: Failed - Reason: {status_data['Failed']}")
                sys.exit(1)
        else:
            print(f"Unexpected status format: {status_data}")
            sys.exit(1)

        time.sleep(poll_interval)

if __name__ == "__main__":
    main()
