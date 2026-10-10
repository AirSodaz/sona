import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import net from "node:net";
import test from "node:test";

const PIPE_NAME =
	process.platform === "win32"
		? `\\\\.\\pipe\\sona-agent-ipc-test-${process.pid}`
		: `/tmp/sona-agent-ipc-test-${process.pid}.sock`;
const OFFLINE_DUMMY_PIPE =
	process.platform === "win32"
		? `\\\\.\\pipe\\sona-agent-ipc-offline-dummy-${process.pid}`
		: `/tmp/sona-agent-ipc-offline-dummy-${process.pid}.sock`;

test("sona-mcp end-to-end integration over IPC pipe", async (_t) => {
	let server;
	let mcp;

	await new Promise((resolve, reject) => {
		server = net.createServer((stream) => {
			let buf = "";
			stream.on("data", (chunk) => {
				buf += chunk.toString();
				const lines = buf.split("\n");
				buf = lines.pop();
				for (const line of lines) {
					if (!line.trim()) continue;
					const req = JSON.parse(line);
					let result = {};
					if (req.method === "sona_get_client_state") {
						result = {
							online: true,
							is_recording: false,
							active_project_id: "p-1",
						};
					} else if (req.method === "sona_start_recording") {
						result = { history_id: "hist-123", started_at: 1700000000 };
					} else if (req.method === "sona_stop_recording") {
						result = {
							history_id: "hist-123",
							duration_seconds: 12.5,
							segment_count: 3,
						};
					} else if (req.method === "sona_query_history") {
						result = [
							{
								id: "hist-123",
								title: "Test Audio",
								preview_text: "Hello world",
								timestamp: 1700000000,
								duration: 12.5,
								project_id: "p-1",
							},
						];
					} else if (req.method === "sona_read_transcript") {
						result = {
							history_id: "hist-123",
							segments: [
								{ id: "seg-1", text: "Hello world", start: 0, end: 2 },
							],
							text: "Hello world",
						};
					} else if (req.method === "sona_edit_transcript") {
						result = { success: true, snapshot_id: "snap-1" };
					} else if (req.method === "sona_delete_history") {
						result = { success: true };
					} else if (req.method === "sona_list_projects") {
						result = [{ id: "p-1", name: "Project 1" }];
					} else if (req.method === "sona_set_active_project") {
						result = { success: true };
					} else if (req.method === "sona_get_settings") {
						result = { theme: "dark" };
					} else if (req.method === "sona_update_setting") {
						result = { success: true };
					} else if (req.method === "sona_focus_window") {
						result = { success: true };
					}
					const resp = { jsonrpc: "2.0", id: req.id, result };
					stream.write(`${JSON.stringify(resp)}\n`);
				}
			});
		});

		server.on("error", reject);
		server.listen(PIPE_NAME, () => {
			resolve();
		});
	});

	mcp = spawn("target/debug/sona-mcp.exe", ["--endpoint", PIPE_NAME]);
	let output = "";
	mcp.stdout.on("data", (d) => {
		output += d.toString();
	});

	function send(msg) {
		return new Promise((resolve) => {
			const mark = output.length;
			mcp.stdin.write(`${JSON.stringify(msg)}\n`);
			const interval = setInterval(() => {
				const fresh = output.slice(mark);
				if (fresh.includes("\n")) {
					clearInterval(interval);
					const line = fresh.trim().split("\n").pop();
					resolve(JSON.parse(line));
				}
			}, 30);
		});
	}

	try {
		// 0. Modern MCP 2026-07-28 server/discover
		const discover = await send({
			jsonrpc: "2.0",
			id: 100,
			method: "server/discover",
			params: {
				_meta: {
					"io.modelcontextprotocol/protocolVersion": "2026-07-28",
					"io.modelcontextprotocol/clientCapabilities": {},
					"io.modelcontextprotocol/clientInfo": {
						name: "IntegrationTestClient",
						version: "1.0.0",
					},
				},
			},
		});
		assert.equal(discover.result.resultType, "complete");
		assert.ok(discover.result.supportedVersions.includes("2026-07-28"));
		assert.equal(discover.result.ttlMs, 3600000);
		assert.equal(discover.result.cacheScope, "public");
		assert.equal(
			discover.result._meta["io.modelcontextprotocol/serverInfo"].name,
			"sona-mcp",
		);

		// 1. initialize (dual-era backward compatibility)
		const init = await send({
			jsonrpc: "2.0",
			id: 1,
			method: "initialize",
			params: {},
		});
		assert.equal(init.result.serverInfo.name, "sona-mcp");
		assert.equal(init.result.protocolVersion, "2026-07-28");
		assert.ok(init.result.capabilities.tools);

		// 2. tools/list (with modern _meta)
		const tools = await send({
			jsonrpc: "2.0",
			id: 2,
			method: "tools/list",
			params: {
				_meta: {
					"io.modelcontextprotocol/protocolVersion": "2026-07-28",
					"io.modelcontextprotocol/clientCapabilities": {},
				},
			},
		});
		assert.equal(tools.result.resultType, "complete");
		assert.equal(tools.result.ttlMs, 300000);
		assert.equal(tools.result.cacheScope, "public");
		assert.equal(tools.result.tools.length, 13);
		assert.equal(tools.result.tools[0].title, "Get Client State");

		// 2b. resources/list
		const resList = await send({
			jsonrpc: "2.0",
			id: 201,
			method: "resources/list",
		});
		assert.equal(resList.result.resultType, "complete");
		assert.equal(resList.result.ttlMs, 300000);
		assert.equal(resList.result.cacheScope, "public");
		assert.equal(resList.result.resources.length, 3);
		assert.equal(resList.result.resources[0].title, "Client Status");
		assert.ok(
			resList.result.resources.some(
				(r) => r.uri === "sona://history/{history_id}",
			),
		);

		// 2c. resources/templates/list
		const resTemplates = await send({
			jsonrpc: "2.0",
			id: 202,
			method: "resources/templates/list",
		});
		assert.equal(resTemplates.result.resultType, "complete");
		assert.equal(resTemplates.result.ttlMs, 300000);
		assert.equal(resTemplates.result.cacheScope, "public");
		assert.equal(resTemplates.result.resourceTemplates.length, 1);
		assert.equal(
			resTemplates.result.resourceTemplates[0].title,
			"History Transcript",
		);
		assert.equal(
			resTemplates.result.resourceTemplates[0].uriTemplate,
			"sona://history/{history_id}",
		);

		// 2d. prompts/list
		const promptList = await send({
			jsonrpc: "2.0",
			id: 203,
			method: "prompts/list",
		});
		assert.equal(promptList.result.resultType, "complete");
		assert.equal(promptList.result.ttlMs, 600000);
		assert.equal(promptList.result.cacheScope, "public");
		assert.equal(promptList.result.prompts.length, 2);
		assert.equal(promptList.result.prompts[0].title, "Summarize Meeting");
		// 3. sona_get_client_state
		const state = await send({
			jsonrpc: "2.0",
			id: 3,
			method: "tools/call",
			params: { name: "sona_get_client_state", arguments: {} },
		});
		const parsedState = JSON.parse(state.result.content[0].text);
		assert.equal(parsedState.online, true);
		assert.equal(parsedState.active_project_id, "p-1");

		// 4. sona_start_recording
		const start = await send({
			jsonrpc: "2.0",
			id: 4,
			method: "tools/call",
			params: { name: "sona_start_recording", arguments: {} },
		});
		const parsedStart = JSON.parse(start.result.content[0].text);
		assert.equal(parsedStart.history_id, "hist-123");

		// 5. sona_stop_recording
		const stop = await send({
			jsonrpc: "2.0",
			id: 5,
			method: "tools/call",
			params: { name: "sona_stop_recording", arguments: {} },
		});
		const parsedStop = JSON.parse(stop.result.content[0].text);
		assert.equal(parsedStop.history_id, "hist-123");
		assert.equal(parsedStop.segment_count, 3);

		// 6. sona_query_history
		const hist = await send({
			jsonrpc: "2.0",
			id: 6,
			method: "tools/call",
			params: { name: "sona_query_history", arguments: { query: "test" } },
		});
		const parsedHist = JSON.parse(hist.result.content[0].text);
		assert.equal(parsedHist.length, 1);
		assert.equal(parsedHist[0].title, "Test Audio");

		// 7. sona_read_transcript
		const trans = await send({
			jsonrpc: "2.0",
			id: 7,
			method: "tools/call",
			params: {
				name: "sona_read_transcript",
				arguments: { history_id: "hist-123" },
			},
		});
		const parsedTrans = JSON.parse(trans.result.content[0].text);
		assert.equal(parsedTrans.text, "Hello world");

		// 8. sona_edit_transcript
		const edit = await send({
			jsonrpc: "2.0",
			id: 8,
			method: "tools/call",
			params: {
				name: "sona_edit_transcript",
				arguments: { history_id: "hist-123", segments: [] },
			},
		});
		const parsedEdit = JSON.parse(edit.result.content[0].text);
		assert.equal(parsedEdit.success, true);
		assert.equal(parsedEdit.snapshot_id, "snap-1");

		// 9. sona_delete_history
		const del = await send({
			jsonrpc: "2.0",
			id: 9,
			method: "tools/call",
			params: {
				name: "sona_delete_history",
				arguments: { history_id: "hist-123" },
			},
		});
		const parsedDel = JSON.parse(del.result.content[0].text);
		assert.equal(parsedDel.success, true);

		// 10. sona_list_projects
		const projs = await send({
			jsonrpc: "2.0",
			id: 10,
			method: "tools/call",
			params: { name: "sona_list_projects", arguments: {} },
		});
		const parsedProjs = JSON.parse(projs.result.content[0].text);
		assert.equal(parsedProjs.length, 1);
		assert.equal(parsedProjs[0].name, "Project 1");

		// 11. sona_set_active_project
		const setp = await send({
			jsonrpc: "2.0",
			id: 11,
			method: "tools/call",
			params: {
				name: "sona_set_active_project",
				arguments: { project_id: "p-1" },
			},
		});
		const parsedSetp = JSON.parse(setp.result.content[0].text);
		assert.equal(parsedSetp.success, true);

		// 12. sona_get_settings
		const getcfg = await send({
			jsonrpc: "2.0",
			id: 12,
			method: "tools/call",
			params: { name: "sona_get_settings", arguments: {} },
		});
		const parsedCfg = JSON.parse(getcfg.result.content[0].text);
		assert.equal(parsedCfg.theme, "dark");

		// 13. sona_update_setting
		const setcfg = await send({
			jsonrpc: "2.0",
			id: 13,
			method: "tools/call",
			params: {
				name: "sona_update_setting",
				arguments: { key: "theme", value: "light" },
			},
		});
		const parsedSetcfg = JSON.parse(setcfg.result.content[0].text);
		assert.equal(parsedSetcfg.success, true);

		// 14. resources/read sona://client/status
		const resStatus = await send({
			jsonrpc: "2.0",
			id: 14,
			method: "resources/read",
			params: { uri: "sona://client/status" },
		});
		const parsedResStatus = JSON.parse(resStatus.result.contents[0].text);
		assert.equal(parsedResStatus.online, true);
		// 14b. resources/read sona://history/hist-123
		const resHist = await send({
			jsonrpc: "2.0",
			id: 141,
			method: "resources/read",
			params: { uri: "sona://history/hist-123" },
		});
		assert.equal(resHist.result.contents[0].text, "Hello world");

		// 15. prompts/get summarize_meeting
		const promptSum = await send({
			jsonrpc: "2.0",
			id: 15,
			method: "prompts/get",
			params: {
				name: "summarize_meeting",
				arguments: { history_id: "hist-123" },
			},
		});
		assert.equal(promptSum.result.messages.length, 1);
		assert.match(promptSum.result.messages[0].content.text, /Requirements/);
		// 15b. prompts/get proofread_transcript
		const promptProof = await send({
			jsonrpc: "2.0",
			id: 151,
			method: "prompts/get",
			params: {
				name: "proofread_transcript",
				arguments: { history_id: "hist-123" },
			},
		});
		assert.equal(promptProof.result.messages.length, 1);
		assert.match(promptProof.result.messages[0].content.text, /proofread/i);

		// 15c. Negative tests for MCP 2026-07-28 error codes
		// Unsupported protocol version -> code -32022
		const unsupported = await send({
			jsonrpc: "2.0",
			id: 901,
			method: "tools/list",
			params: {
				_meta: {
					"io.modelcontextprotocol/protocolVersion": "1999-01-01",
					"io.modelcontextprotocol/clientCapabilities": {},
				},
			},
		});
		assert.equal(unsupported.error.code, -32022);
		assert.ok(unsupported.error.data.supported.includes("2026-07-28"));

		// Missing clientCapabilities when _meta is present -> code -32602
		const missingCaps = await send({
			jsonrpc: "2.0",
			id: 902,
			method: "tools/list",
			params: {
				_meta: {
					"io.modelcontextprotocol/protocolVersion": "2026-07-28",
				},
			},
		});
		assert.equal(missingCaps.error.code, -32602);

		// Resource not found -> code -32602 per 2026-07-28 spec
		const notFoundRes = await send({
			jsonrpc: "2.0",
			id: 903,
			method: "resources/read",
			params: { uri: "sona://nonexistent" },
		});
		assert.equal(notFoundRes.error.code, -32602);

		// Prompt not found -> code -32602 per 2026-07-28 spec
		const notFoundPrompt = await send({
			jsonrpc: "2.0",
			id: 904,
			method: "prompts/get",
			params: { name: "nonexistent", arguments: { history_id: "h-1" } },
		});
		assert.equal(notFoundPrompt.error.code, -32602);
	} finally {
		mcp.kill();
		server.close();
	}
});
test("sona-mcp offline handling returns offline state and clear offline errors", async () => {
	const mcp = spawn("target/debug/sona-mcp.exe", [
		"--endpoint",
		OFFLINE_DUMMY_PIPE,
	]);
	let output = "";
	mcp.stdout.on("data", (d) => {
		output += d.toString();
	});

	function send(msg) {
		return new Promise((resolve) => {
			const mark = output.length;
			mcp.stdin.write(`${JSON.stringify(msg)}\n`);
			const interval = setInterval(() => {
				const fresh = output.slice(mark);
				if (fresh.includes("\n")) {
					clearInterval(interval);
					const line = fresh.trim().split("\n").pop();
					resolve(JSON.parse(line));
				}
			}, 30);
		});
	}

	try {
		// 1. sona_get_client_state when desktop is offline
		const state = await send({
			jsonrpc: "2.0",
			id: 1,
			method: "tools/call",
			params: { name: "sona_get_client_state", arguments: {} },
		});
		const parsedState = JSON.parse(state.result.content[0].text);
		assert.equal(parsedState.online, false);
		assert.equal(parsedState.is_recording, false);
		assert.equal(parsedState.active_project_id, null);

		// 2. other tool calls return DESKTOP_OFFLINE error
		const record = await send({
			jsonrpc: "2.0",
			id: 2,
			method: "tools/call",
			params: { name: "sona_start_recording", arguments: {} },
		});
		assert.equal(record.result.isError, true);
		assert.match(record.result.content[0].text, /DESKTOP_OFFLINE/);
	} finally {
		mcp.kill();
	}
});
