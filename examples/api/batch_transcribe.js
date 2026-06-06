#!/usr/bin/env node
/**
 * Sona API Batch Transcription Example
 * Submits a local audio/video file to Sona API, polls the status, and prints segments.
 * Requires Node.js 18+ (for native fetch and FormData)
 */

import fs from 'fs';
import path from 'path';

/**
 * Helper function to perform fetch requests with a timeout
 * @param {string} url
 * @param {object} options
 * @param {number} timeoutMs
 * @returns {Promise<Response>}
 */
async function fetchWithTimeout(url, options = {}, timeoutMs = 30000) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal
    });
    return response;
  } catch (err) {
    if (err.name === 'AbortError') {
      throw new Error(`Request timed out after ${timeoutMs / 1000}s`);
    }
    throw err;
  } finally {
    clearTimeout(timeoutId);
  }
}

/**
 * Helper function to sleep for a specified duration in milliseconds
 * @param {number} ms
 * @returns {Promise<void>}
 */
function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 2) {
    console.log("Usage: node batch_transcribe.js <file_path> <model_id> [api_key] [host] [port]");
    process.exit(1);
  }

  const filePath = args[0];
  const modelId = args[1];
  const apiKey = args[2] || "";
  const host = args[3] || "127.0.0.1";
  const port = args[4] || "14200";

  if (!fs.existsSync(filePath)) {
    console.error(`Error: File not found at ${filePath}`);
    process.exit(1);
  }

  const baseUrl = `http://${host}:${port}`;
  const headers = {};
  if (apiKey) {
    headers["Authorization"] = `Bearer ${apiKey}`;
  }

  // 1. Submit Transcription Job
  const uploadUrl = `${baseUrl}/v1/transcriptions`;
  console.log(`Uploading ${filePath} to Sona API...`);

  let jobId;
  try {
    const fileStream = fs.createReadStream(filePath);
    const formData = new FormData();
    // Wrap the read stream into a Blob-like object so that FormData correctly handles it
    const fileStats = fs.statSync(filePath);
    const fileBlob = {
      [Symbol.toStringTag]: 'File',
      name: path.basename(filePath),
      size: fileStats.size,
      stream: () => fileStream,
      arrayBuffer: async () => {
        const buffers = [];
        for await (const chunk of fileStream) {
          buffers.push(chunk);
        }
        return Buffer.concat(buffers).buffer;
      }
    };

    formData.append('file', fileBlob, path.basename(filePath));
    formData.append('model_id', modelId);
    formData.append('language', 'auto');

    const response = await fetchWithTimeout(uploadUrl, {
      method: 'POST',
      headers: headers,
      body: formData
    });

    if (!response.ok) {
      const errText = await response.text();
      console.error(`Error submitting job (HTTP ${response.status}): ${errText}`);
      process.exit(1);
    }

    const jobInfo = await response.json();
    jobId = jobInfo.job_id;
    if (!jobId) {
      console.error("Error: Job ID (job_id) is missing in server response:", jobInfo);
      process.exit(1);
    }
    console.log(`Job successfully submitted! Job ID: ${jobId}`);
  } catch (err) {
    console.error("Failed to submit transcription job:", err);
    process.exit(1);
  }

  // 2. Poll Job Status
  const statusUrl = `${baseUrl}/v1/transcriptions/${jobId}`;
  console.log("Polling job status...");

  const pollInterval = 2000;
  let consecutiveErrors = 0;
  const maxConsecutiveErrors = 5;

  while (true) {
    try {
      const response = await fetchWithTimeout(statusUrl, { headers });
      if (!response.ok) {
        console.error(`Error querying job status (HTTP ${response.status})`);
        process.exit(1);
      }

      const statusData = await response.json();
      consecutiveErrors = 0; // Reset consecutive errors on successful response

      if (statusData === "Pending") {
        console.log("Status: Pending (in queue)...");
      } else if (statusData === "Processing") {
        console.log("Status: Processing...");
      } else if (typeof statusData === "object" && statusData !== null) {
        if ("Completed" in statusData) {
          const segments = statusData.Completed;
          console.log("\nTranscription Completed Successfully!");
          console.log("=".repeat(60));
          for (const seg of segments) {
            const startTime = seg.start || 0;
            const endTime = seg.end || 0;
            const text = seg.text || "";
            const speaker = seg.speaker || "Unknown";
            console.log(`[${startTime.toFixed(2)}s - ${endTime.toFixed(2)}s] (${speaker}): ${text}`);
          }
          console.log("=".repeat(60));
          break;
        } else if ("Failed" in statusData) {
          console.error(`Status: Failed - Reason: ${statusData.Failed}`);
          process.exit(1);
        } else {
          console.error("Unexpected object in status response:", statusData);
          process.exit(1);
        }
      } else {
        console.error("Unexpected status format:", statusData);
        process.exit(1);
      }
    } catch (err) {
      consecutiveErrors++;
      console.warn(`Warning: Polling request encountered a transient error (attempt ${consecutiveErrors}/${maxConsecutiveErrors}): ${err.message || err}`);
      if (consecutiveErrors >= maxConsecutiveErrors) {
        console.error("Failed after 5 consecutive transient network errors. Exiting.");
        process.exit(1);
      }
    }

    await sleep(pollInterval);
  }
}

main().catch((err) => {
  console.error("Unexpected error in main:", err);
  process.exit(1);
});
