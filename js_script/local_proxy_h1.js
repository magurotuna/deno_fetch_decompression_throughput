const PORT = 24444;
const UPSTREAM_PORT = 3111;

const client = Deno.createHttpClient({ http1: true, http2: false });

Deno.serve({ port: PORT }, async (_req) => {
  const url = `http://localhost:${UPSTREAM_PORT}`;
  const resp = await fetch(url, { client });
  return new Response(resp.body, { headers: resp.headers });
});
