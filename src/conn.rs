use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use hyper_util::rt::TokioIo;

use crate::Args;

pub trait HyperIo: hyper::rt::Read + hyper::rt::Write + Unpin + Send {}

impl<T: hyper::rt::Read + hyper::rt::Write + Unpin + Send> HyperIo for T {}

pub async fn get_conn(args: &Args) -> anyhow::Result<Box<dyn HyperIo>> {
    let conn: Box<dyn HyperIo> = match (args.server_port, &args.unix_socket_path) {
        (Some(server_port), None) => {
            let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, server_port));
            let io = TokioIo::new(tokio::net::TcpStream::connect(addr).await?);
            Box::new(io)
        }
        (None, Some(unix_socket_path)) => {
            let io = TokioIo::new(tokio::net::UnixStream::connect(unix_socket_path).await?);
            Box::new(io)
        }
        _ => anyhow::bail!("Either server port or unix socket path must be specified"),
    };

    Ok(conn)
}
