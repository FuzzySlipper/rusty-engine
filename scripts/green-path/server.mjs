// The green-path check's pages, and the log their reports go to.
// usage: node server.mjs <report log>   prints the port it listens on (127.0.0.1)
import { createServer } from 'node:http';
import { readFile, appendFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const pages = join(fileURLToPath(new URL('.', import.meta.url)), 'pages');
const log = process.argv[2];
createServer(async (request, response) => {
  if (request.method === 'POST') {
    let body = '';
    for await (const chunk of request) body += chunk;
    await appendFile(log, body + '\n');
    response.end();
    return;
  }
  const name = new URL(request.url, 'http://page').pathname.slice(1) || 'audit';
  try {
    const page = await readFile(join(pages, `${name.replace(/[^a-z]/g, '')}.html`));
    response.writeHead(200, { 'content-type': 'text/html' });
    response.end(page);
  } catch {
    response.writeHead(404);
    response.end();
  }
}).listen(0, '127.0.0.1', function () {
  console.log(this.address().port);
});
