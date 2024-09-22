use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use clap::Parser;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use tokio::net::TcpListener;

const RAW_DATA: Bytes = Bytes::from_static(include_bytes!("./large_data.txt"));
const COMPRESSED_DATA: Bytes = Bytes::from_static(include_bytes!("./large_data.txt.br"));

async fn serve_data(
    _: Request<impl hyper::body::Body>,
    compress: bool,
) -> Result<Response<Full<Bytes>>, Infallible> {
    const COMMON_HEADER_NAME: http::header::HeaderName =
        http::header::HeaderName::from_static("x-super-fast-large-data-server");

    let resp = if compress {
        let body = Full::new(COMPRESSED_DATA);
        let resp = Response::builder()
            .header(http::header::CONTENT_ENCODING, "br")
            .header(COMMON_HEADER_NAME, "true")
            .body(body)
            .unwrap();
        resp
    } else {
        let body = Full::new(RAW_DATA);
        let resp = Response::builder()
            .header(COMMON_HEADER_NAME, "true")
            .body(body)
            .unwrap();
        resp
    };
    Ok(resp)
}

#[derive(Parser, Debug)]
struct Args {
    /// Whether to compress the response body with Brotli
    #[arg(long, default_value = "false")]
    compress: bool,
    /// Whether to use HTTP/1.1
    /// If false, HTTP/2 will be used
    #[arg(long, default_value = "false")]
    http1: bool,
    #[arg(long, default_value = "3111")]
    port: u16,
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    let args = Args::parse();

    let addr: SocketAddr = ([127, 0, 0, 1], args.port).into();

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        let timer = TokioTimer::new();

        tokio::task::spawn({
            async move {
                if args.http1 {
                    if let Err(err) = http1::Builder::new()
                        .timer(timer)
                        .serve_connection(
                            io,
                            service_fn(|req| async move { serve_data(req, args.compress).await }),
                        )
                        .await
                    {
                        println!("Error serving connection: {:?}", err);
                    }
                } else {
                    if let Err(err) = http2::Builder::new(TokioExecutor::new())
                        .timer(timer)
                        .serve_connection(
                            io,
                            service_fn(|req| async move { serve_data(req, args.compress).await }),
                        )
                        .await
                    {
                        println!("Error serving connection: {:?}", err);
                    }
                }
            }
        });
    }
}
