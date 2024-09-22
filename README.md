# deno_fetch_decompression_throughput

This repository contains several components to measure the throughput of a proxy
server written in Deno.

## Components

### Upstream server

The upstream server is a simple HTTP server implemented with `hyper`.

```console
Usage: super_fast_large_data_server [OPTIONS]

Options:
      --compress     Whether to compress the response body with Brotli
      --http1        Whether to use HTTP/1.1 If false, HTTP/2 will be used
      --port <PORT>  [default: 3111]
  -h, --help         Print help
```

Key properties:

- It can serve either a large text file (2.0MB) or a brotli-compressed version
  of the same data (6.3KB), depending on the `--compressed` flag.
- It can talk either HTTP/1.1 or HTTP/2, depending on the `--http1` flag.

### Proxy server

The proxy server is written in Deno, simply `fetch`ing the data from the
upstream server and forwarding it to the end client.

It is as simple as:

```ts
Deno.serve({ port: PORT }, async (_req) => {
  const url = `http://localhost:${UPSTREAM_PORT}`;
  const resp = await fetch(url);
  return new Response(resp.body, { headers: resp.headers });
});
```

### Traffic generator

Finally, we have a traffic generator that sends a large amount of traffic to the
proxy server.

```console
Usage: deno_fetch_decompression_throughput [OPTIONS] --concurrency <CONCURRENCY> --iterations <ITERATIONS>

Options:
      --deno-path <DENO_PATH>
          Path to the deno binary If not provided, it is assumed that the proxy server is already running
      --proxy-script-path <PROXY_SCRIPT_PATH>
          Path to the proxy server script to run on deno
      --server-port <SERVER_PORT>
          The port number that the proxy server will listen on. The traffic generator will connect to this server on this port. Either this or `unix_socket_path` must be provided
      --concurrency <CONCURRENCY>

      --iterations <ITERATIONS>

      --use-http1
          If set, the traffic generator will use HTTP/1.1 instead of HTTP/2 to send requests to the proxy server
      --unix-socket-path <UNIX_SOCKET_PATH>
          The path to the Unix socket file that the traffic generator will connect to. Either this or `server_port` must be provided
      --interactive
          If set, the traffic generator will prompt the user before sending traffic to the proxy server
  -h, --help
          Print help
```

## How to measure the throughput

1. Place the Deno binary that you want to measure the throughput against in the
   `deno_bin` directory.
   - `deno upgrade` command is convenient e.g.

```console
deno upgrade --version 1.45.3 --output deno_bin/1_45_3.bin
```

2. Start the upstream server, e.g.

```console
cargo run --release --example super_fast_large_data_server -- --compress
```

3. Run the traffic generator (note: the proxy server is spawned by the traffic
   generator) e.g.

```console
cargo run --release -- --concurrency 300 --iterations 2000 \
  --deno-path ./deno_bin/1_45_3.bin \
  --proxy-script-path ./js_script/local_proxy_h2.js \
  --server-port 24444
```

Or with [hyperfine](https://github.com/sharkdp/hyperfine):

```console
hyperfine --warmup 1 --runs 5 --export-json 1_45_3.json \
'cargo run --release -- --concurrency 300 --iterations 2000 \
  --deno-path ./deno_bin/1_45_3.bin \
  --proxy-script-path ./js_script/local_proxy_h2.js \
  --server-port 24444'
```

## Results

| Parameter   | Value |
| ----------- | ----- |
| concurrency | 300   |
| iterations  | 2000  |
