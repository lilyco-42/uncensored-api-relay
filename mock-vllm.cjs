// mock-vllm.cjs — 忠于 vLLM OpenAI 行为的本地假上游，用于离线自测中转站
// 不需要真实上游就能验证：双协议、模型映射、SSE 流式、鉴权头透传。
//
// 用法：
//   node mock-vllm.cjs                       # 默认 127.0.0.1:9911
//   $env:UPSTREAM_URL="http://127.0.0.1:9911/v1"; .\uncensored-proxy.exe
//

const http = require('http');
const srv = http.createServer((req, res) => {
  let body = '';
  req.on('data', d => body += d.toString());
  req.on('end', () => {
    const url = new URL(req.url, 'http://x');
    if (url.pathname === '/v1/models') {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ object: 'list', data: [{ id: 'uncensored-27b', object: 'model' }] }));
      return;
    }
    if (url.pathname === '/v1/chat/completions' || url.pathname === '/v1/messages') {
      let obj = {}; try { obj = JSON.parse(body); } catch {}
      const stream = !!obj.stream;
      const model = obj.model;
      const txt = 'mock-uncensored-ok';
      if (!stream) {
        if (url.pathname === '/v1/chat/completions') {
          res.writeHead(200, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ id: 'c1', object: 'chat.completion', model,
            choices: [{ index: 0, message: { role: 'assistant', content: txt }, finish_reason: 'stop' }],
            usage: { prompt_tokens: 5, completion_tokens: 3, total_tokens: 8 } }));
        } else {
          res.writeHead(200, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ id: 'm1', type: 'message', role: 'assistant', model,
            content: [{ type: 'text', text: txt }], stop_reason: 'end_turn',
            usage: { input_tokens: 5, output_tokens: 3 } }));
        }
        return;
      }
      res.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' });
      const chunks = txt.split('');
      let i = 0;
      const tick = setInterval(() => {
        if (i >= chunks.length) { res.write('data: [DONE]\n\n'); clearInterval(tick); res.end(); return; }
        const piece = chunks[i++];
        if (url.pathname === '/v1/chat/completions') {
          res.write('data: ' + JSON.stringify({ choices: [{ index: 0, delta: { content: piece } }] }) + '\n\n');
        } else {
          res.write('event: content_block_delta\ndata: ' + JSON.stringify({ type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text: piece } }) + '\n\n');
        }
      }, 15);
      return;
    }
    res.writeHead(404, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ error: 'not found' }));
  });
});
srv.listen(9911, '127.0.0.1', () => console.log('mock vLLM on 127.0.0.1:9911'));
