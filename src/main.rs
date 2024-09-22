use clap::Parser;
use futures::stream::StreamExt as _;
use hyper_util::rt::TokioExecutor;
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, BufReader};

mod conn;
mod traffic;

#[derive(Parser, Debug)]
struct Args {
    /// Path to the deno binary
    /// If not provided, it is assumed that the proxy server is already running.
    #[arg(long)]
    deno_path: Option<String>,
    /// Path to the proxy server script to run on deno.
    #[arg(long)]
    proxy_script_path: Option<String>,
    /// The port number that the proxy server will listen on.
    /// The traffic generator will connect to this server on this port.
    /// Either this or `unix_socket_path` must be provided.
    #[arg(long)]
    server_port: Option<u16>,
    #[arg(long)]
    concurrency: usize,
    #[arg(long)]
    iterations: usize,
    /// If set, the traffic generator will use HTTP/1.1 instead of HTTP/2 to
    /// send requests to the proxy server.
    #[arg(long, default_value = "false")]
    use_http1: bool,
    /// The path to the Unix socket file that the traffic generator will connect to.
    /// Either this or `server_port` must be provided.
    #[arg(long)]
    unix_socket_path: Option<String>,
    /// If set, the traffic generator will prompt the user before sending
    /// traffic to the proxy server.
    #[arg(long, default_value = "false")]
    interactive: bool,
}

fn main() {
    pretty_env_logger::init();

    let args = Args::parse();

    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(inner_main(args));

    if let Err(e) = result {
        log::error!("Error: {e:?}");
        std::process::exit(1);
    }
}

async fn inner_main(args: Args) -> anyhow::Result<()> {
    let _deno_proc = match (&args.deno_path, &args.proxy_script_path) {
        (Some(deno_path), Some(script_path)) => {
            let deno_proc = spawn_deno(deno_path, script_path).await?;
            println!("Deno process spawned: pid = {}", deno_proc.id().unwrap());
            Some(deno_proc)
        }
        _ => {
            println!("Use the already running deno server");
            None
        }
    };

    let conn = conn::get_conn(&args).await?;

    if args.use_http1 {
        let (http1_client, conn_driver) = hyper::client::conn::http1::handshake(conn).await?;
        tokio::spawn(async move {
            if let Err(e) = conn_driver.await {
                log::error!("Connection driver error: {e:?}");
            }
        });

        if args.interactive {
            prompt().await?;
        }

        traffic::send_traffic_h1(
            http1_client,
            "http://localhost".to_string(),
            args.concurrency,
            args.iterations,
        )
        .await?;
    } else {
        let (http2_client, conn_driver) =
            hyper::client::conn::http2::handshake(TokioExecutor::new(), conn).await?;
        tokio::spawn(async move {
            if let Err(e) = conn_driver.await {
                log::error!("Connection driver error: {e:?}");
            }
        });

        if args.interactive {
            prompt().await?;
        }

        traffic::send_traffic(
            http2_client,
            "http://localhost".to_string(),
            args.concurrency,
            args.iterations,
        )
        .await?;
    }

    println!("All done!");

    Ok(())
}

async fn prompt() -> anyhow::Result<()> {
    println!("✅ Press any key to start sending HTTP requests to the server...");
    tokio::io::stdin().read_exact(&mut [0]).await?;
    Ok(())
}

async fn spawn_deno(deno_path: &str, script_path: &str) -> anyhow::Result<tokio::process::Child> {
    let mut proc = tokio::process::Command::new(deno_path)
        .arg("run")
        .arg("--unstable-http")
        .arg("-A")
        .arg(script_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    // NOTE: Since v2.0 rc version `Deno.serve` emits the readiness message to
    // stderr instead of stdout. To make sure we can properly wait until the
    // server is ready regardless of the Deno version, we inspect both stdout
    // and stderr.
    // https://github.com/denoland/deno/pull/25491
    let stdout = BufReader::new(proc.stdout.take().unwrap());
    let stdout_line_stream = tokio_stream::wrappers::LinesStream::new(stdout.lines());
    let stderr = BufReader::new(proc.stderr.take().unwrap());
    let stderr_line_stream = tokio_stream::wrappers::LinesStream::new(stderr.lines());
    let merged_stream = futures::stream::select(stdout_line_stream, stderr_line_stream);

    tokio::spawn(async move {
        let mut merged_stream = std::pin::pin!(merged_stream);

        while let Some(line) = merged_stream.as_mut().next().await {
            match line {
                Ok(line) => {
                    println!("{line}");
                    if line.starts_with("Listening on ") {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Error reading line: {e:?}");
                    anyhow::bail!("Error reading line");
                }
            }
        }

        Ok(())
    })
    .await??;

    Ok(proc)
}
