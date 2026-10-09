import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import net from 'node:net';
import test from 'node:test';

const PIPE_NAME = '\\\\.\\pipe\\sona-agent-ipc';

test('sona-mcp end-to-end integration over IPC pipe', async (t) => {
  let server;
  let mcp;

  await new Promise((resolve, reject) => {
    server = net.createServer((stream) => {
      let buf = '';
      stream.on('data', (chunk) => {
        buf += chunk.toString();
        const lines = buf.split('\n');
        buf = lines.pop();
        for (const line of lines) {
          if (!line.trim()) continue;
          const req = JSON.parse(line);
          let result = {};
          if (req.method === 'sona_get_client_state') {
            result = { online: true, is_recording: false, active_project_id: 'p-1' };
          } else if (req.method === 'sona_start_recording') {
            result = { history_id: 'hist-123', started_at: 1700000000 };
          } else if (req.method === 'sona_stop_recording') {
            result = { history_id: 'hist-123', duration_seconds: 12.5, segment_count: 3 };
          } else if (req.method === 'sona_query_history') {
            result = [
              {
                id: 'hist-123',
                title: 'Test Audio',
                preview_text: 'Hello world',
                timestamp: 1700000000,
                duration: 12.5,
                project_id: 'p-1',
              },
            ];
          } else if (req.method === 'sona_read_transcript') {
            result = {
              history_id: 'hist-123',
              segments: [{ id: 'seg-1', text: 'Hello world', start: 0, end: 2 }],
              text: 'Hello world',
            };
          } else if (req.method === 'sona_edit_transcript') {
            result = { success: true, snapshot_id: 'snap-1' };
          } else if (req.method === 'sona_delete_history') {
            result = { success: true };
          } else if (req.method === 'sona_list_projects') {
            result = [{ id: 'p-1', name: 'Project 1' }];
          } else if (req.method === 'sona_set_active_project') {
            result = { success: true };
          } else if (req.method === 'sona_get_settings') {
            result = { theme: 'dark' };
          } else if (req.method === 'sona_update_setting') {
            result = { success: true };
          } else if (req.method === 'sona_focus_window') {
            result = { success: true };
          }
          const resp = { jsonrpc: '2.0', id: req.id, result };
          stream.write(JSON.stringify(resp) + '\n');
        }
      });
    });

    server.on('error', reject);
    server.listen(PIPE_NAME, () => {
      resolve();
    });
  });

  mcp = spawn('target/debug/sona-mcp.exe');
  let output = '';
  mcp.stdout.on('data', (d) => {
    output += d.toString();
  });

  function send(msg) {
    return new Promise((resolve) => {
      const mark = output.length;
      mcp.stdin.write(JSON.stringify(msg) + '\n');
      const interval = setInterval(() => {
        const fresh = output.slice(mark);
        if (fresh.includes('\n')) {
          clearInterval(interval);
          const line = fresh.trim().split('\n').pop();
          resolve(JSON.parse(line));
        }
      }, 30);
    });
  }

  try {
    // 1. initialize
    const init = await send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: {} });
    assert.equal(init.result.serverInfo.name, 'sona-mcp');
    assert.ok(init.result.capabilities.tools);

    // 2. tools/list
    const tools = await send({ jsonrpc: '2.0', id: 2, method: 'tools/list' });
    assert.equal(tools.result.tools.length, 13);
    // 2b. resources/list
    const resList = await send({ jsonrpc: '2.0', id: 201, method: 'resources/list' });
    assert.equal(resList.result.resources.length, 3);
    assert.ok(resList.result.resources.some((r) => r.uri === 'sona://history/{history_id}'));

    // 2c. resources/templates/list
    const resTemplates = await send({ jsonrpc: '2.0', id: 202, method: 'resources/templates/list' });
    assert.equal(resTemplates.result.resourceTemplates.length, 1);
    assert.equal(resTemplates.result.resourceTemplates[0].uriTemplate, 'sona://history/{history_id}');

    // 2d. prompts/list
    const promptList = await send({ jsonrpc: '2.0', id: 203, method: 'prompts/list' });
    assert.equal(promptList.result.prompts.length, 2);

    // 3. sona_get_client_state
    const state = await send({
      jsonrpc: '2.0',
      id: 3,
      method: 'tools/call',
      params: { name: 'sona_get_client_state', arguments: {} },
    });
    const parsedState = JSON.parse(state.result.content[0].text);
    assert.equal(parsedState.online, true);
    assert.equal(parsedState.active_project_id, 'p-1');

    // 4. sona_start_recording
    const start = await send({
      jsonrpc: '2.0',
      id: 4,
      method: 'tools/call',
      params: { name: 'sona_start_recording', arguments: {} },
    });
    const parsedStart = JSON.parse(start.result.content[0].text);
    assert.equal(parsedStart.history_id, 'hist-123');

    // 5. sona_stop_recording
    const stop = await send({
      jsonrpc: '2.0',
      id: 5,
      method: 'tools/call',
      params: { name: 'sona_stop_recording', arguments: {} },
    });
    const parsedStop = JSON.parse(stop.result.content[0].text);
    assert.equal(parsedStop.history_id, 'hist-123');
    assert.equal(parsedStop.segment_count, 3);

    // 6. sona_query_history
    const hist = await send({
      jsonrpc: '2.0',
      id: 6,
      method: 'tools/call',
      params: { name: 'sona_query_history', arguments: { query: 'test' } },
    });
    const parsedHist = JSON.parse(hist.result.content[0].text);
    assert.equal(parsedHist.length, 1);
    assert.equal(parsedHist[0].title, 'Test Audio');

    // 7. sona_read_transcript
    const trans = await send({
      jsonrpc: '2.0',
      id: 7,
      method: 'tools/call',
      params: { name: 'sona_read_transcript', arguments: { history_id: 'hist-123' } },
    });
    const parsedTrans = JSON.parse(trans.result.content[0].text);
    assert.equal(parsedTrans.text, 'Hello world');

    // 8. sona_edit_transcript
    const edit = await send({
      jsonrpc: '2.0',
      id: 8,
      method: 'tools/call',
      params: { name: 'sona_edit_transcript', arguments: { history_id: 'hist-123', segments: [] } },
    });
    const parsedEdit = JSON.parse(edit.result.content[0].text);
    assert.equal(parsedEdit.success, true);
    assert.equal(parsedEdit.snapshot_id, 'snap-1');

    // 9. sona_delete_history
    const del = await send({
      jsonrpc: '2.0',
      id: 9,
      method: 'tools/call',
      params: { name: 'sona_delete_history', arguments: { history_id: 'hist-123' } },
    });
    const parsedDel = JSON.parse(del.result.content[0].text);
    assert.equal(parsedDel.success, true);

    // 10. sona_list_projects
    const projs = await send({
      jsonrpc: '2.0',
      id: 10,
      method: 'tools/call',
      params: { name: 'sona_list_projects', arguments: {} },
    });
    const parsedProjs = JSON.parse(projs.result.content[0].text);
    assert.equal(parsedProjs.length, 1);
    assert.equal(parsedProjs[0].name, 'Project 1');

    // 11. sona_set_active_project
    const setp = await send({
      jsonrpc: '2.0',
      id: 11,
      method: 'tools/call',
      params: { name: 'sona_set_active_project', arguments: { project_id: 'p-1' } },
    });
    const parsedSetp = JSON.parse(setp.result.content[0].text);
    assert.equal(parsedSetp.success, true);

    // 12. sona_get_settings
    const getcfg = await send({
      jsonrpc: '2.0',
      id: 12,
      method: 'tools/call',
      params: { name: 'sona_get_settings', arguments: {} },
    });
    const parsedCfg = JSON.parse(getcfg.result.content[0].text);
    assert.equal(parsedCfg.theme, 'dark');

    // 13. sona_update_setting
    const setcfg = await send({
      jsonrpc: '2.0',
      id: 13,
      method: 'tools/call',
      params: { name: 'sona_update_setting', arguments: { key: 'theme', value: 'light' } },
    });
    const parsedSetcfg = JSON.parse(setcfg.result.content[0].text);
    assert.equal(parsedSetcfg.success, true);

    // 14. resources/read sona://client/status
    const resStatus = await send({
      jsonrpc: '2.0',
      id: 14,
      method: 'resources/read',
      params: { uri: 'sona://client/status' },
    });
    const parsedResStatus = JSON.parse(resStatus.result.contents[0].text);
    assert.equal(parsedResStatus.online, true);
    // 14b. resources/read sona://history/hist-123
    const resHist = await send({
      jsonrpc: '2.0',
      id: 141,
      method: 'resources/read',
      params: { uri: 'sona://history/hist-123' },
    });
    assert.equal(resHist.result.contents[0].text, 'Hello world');

    // 15. prompts/get summarize_meeting
    const promptSum = await send({
      jsonrpc: '2.0',
      id: 15,
      method: 'prompts/get',
      params: { name: 'summarize_meeting', arguments: { history_id: 'hist-123' } },
    });
    assert.equal(promptSum.result.messages.length, 1);
    assert.match(promptSum.result.messages[0].content.text, /Requirements/);
    // 15b. prompts/get proofread_transcript
    const promptProof = await send({
      jsonrpc: '2.0',
      id: 151,
      method: 'prompts/get',
      params: { name: 'proofread_transcript', arguments: { history_id: 'hist-123' } },
    });
    assert.equal(promptProof.result.messages.length, 1);
    assert.match(promptProof.result.messages[0].content.text, /proofread/i);
  } finally {
    mcp.kill();
    server.close();
  }
});
test('sona-mcp offline handling returns offline state and clear offline errors', async () => {
  const mcp = spawn('target/debug/sona-mcp.exe');
  let output = '';
  mcp.stdout.on('data', (d) => {
    output += d.toString();
  });

  function send(msg) {
    return new Promise((resolve) => {
      const mark = output.length;
      mcp.stdin.write(JSON.stringify(msg) + '\n');
      const interval = setInterval(() => {
        const fresh = output.slice(mark);
        if (fresh.includes('\n')) {
          clearInterval(interval);
          const line = fresh.trim().split('\n').pop();
          resolve(JSON.parse(line));
        }
      }, 30);
    });
  }

  try {
    // 1. sona_get_client_state when desktop is offline
    const state = await send({
      jsonrpc: '2.0',
      id: 1,
      method: 'tools/call',
      params: { name: 'sona_get_client_state', arguments: {} },
    });
    const parsedState = JSON.parse(state.result.content[0].text);
    assert.equal(parsedState.online, false);
    assert.equal(parsedState.is_recording, false);
    assert.equal(parsedState.active_project_id, null);

    // 2. other tool calls return DESKTOP_OFFLINE error
    const record = await send({
      jsonrpc: '2.0',
      id: 2,
      method: 'tools/call',
      params: { name: 'sona_start_recording', arguments: {} },
    });
    assert.equal(record.result.isError, true);
    assert.match(record.result.content[0].text, /DESKTOP_OFFLINE/);
  } finally {
    mcp.kill();
  }
});
