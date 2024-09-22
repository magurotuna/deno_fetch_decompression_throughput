use std::sync::Arc;

use futures::StreamExt as _;
use http_body_util::{BodyExt as _, Empty};
use hyper::{
    body::Bytes,
    client::conn::{http1, http2},
};

pub async fn send_traffic_h1(
    mut client: http1::SendRequest<Empty<Bytes>>,
    url: String,
    concurrency: usize,
    iterations: usize,
) -> anyhow::Result<()> {
    // Send one request to warm up the server.
    send_one_req_h1(&mut client, &url, usize::MAX).await?;

    let client = Arc::new(tokio::sync::Mutex::new(client));

    let bar = indicatif::ProgressBar::new(iterations as u64);

    let mut req_stream = futures::stream::iter((0..iterations).map(|i| {
        let client = client.clone();
        let url = url.clone();
        async move {
            let mut client = client.lock().await;
            send_one_req_h1(&mut client, &url, i).await
        }
    }))
    .inspect(|_| bar.inc(1))
    .buffer_unordered(concurrency);

    while let Some(res) = req_stream.next().await {
        if let Err(e) = res {
            return Err(e.context("error sending request"));
        }
    }

    bar.finish();

    Ok(())
}

async fn send_one_req_h1(
    client: &mut http1::SendRequest<Empty<Bytes>>,
    url: &str,
    req_seq_id: usize,
) -> anyhow::Result<()> {
    let req = http::Request::builder()
        .uri(url)
        .body(Empty::<Bytes>::new())?;
    let res = client.send_request(req).await.inspect_err(|e| {
        log::error!("error sending request {req_seq_id}: {e:?}");
    })?;
    let (parts, body) = res.into_parts();
    let mut res_body_stream = body.into_data_stream();
    let mut size = 0;
    while let Some(chunk) = res_body_stream.next().await {
        match chunk {
            Ok(chunk) => {
                size += chunk.len();
            }
            Err(e) => {
                log::error!("error reading response body: {e:?}");
                break;
            }
        }
    }
    assert_eq!(size, 1069320);
    assert_eq!(
        parts.headers.get("content-type").unwrap(),
        &"application/wasm"
    );
    Ok(())
}

pub async fn send_traffic(
    client: http2::SendRequest<Empty<Bytes>>,
    url: String,
    concurrency: usize,
    iterations: usize,
) -> anyhow::Result<()> {
    // Send one request to warm up the server.
    send_one_req(client.clone(), &url, usize::MAX).await?;

    let bar = indicatif::ProgressBar::new(iterations as u64);

    let mut req_stream = futures::stream::iter((0..iterations).map(|i| {
        let client = client.clone();
        send_one_req(client, &url, i)
    }))
    .inspect(|_| bar.inc(1))
    .buffer_unordered(concurrency);

    while let Some(res) = req_stream.next().await {
        if let Err(e) = res {
            return Err(e.context("error sending request"));
        }
    }

    bar.finish();

    Ok(())
}

async fn send_one_req(
    mut client: http2::SendRequest<Empty<Bytes>>,
    url: &str,
    req_seq_id: usize,
) -> anyhow::Result<()> {
    let req = http::Request::builder()
        .uri(url)
        .body(Empty::<Bytes>::new())?;
    let res = client.send_request(req).await.inspect_err(|e| {
        log::error!("error sending request {req_seq_id}: {e:?}");
    })?;
    let (parts, body) = res.into_parts();

    // Check the response header
    const UPSTREAM_HEADER_NAME: http::header::HeaderName =
        http::header::HeaderName::from_static("x-super-fast-large-data-server");
    const UPSTREAM_HEADER_EXPECTED_VALUE: http::header::HeaderValue =
        http::header::HeaderValue::from_static("true");
    assert_eq!(
        parts.headers.get(UPSTREAM_HEADER_NAME),
        Some(&UPSTREAM_HEADER_EXPECTED_VALUE)
    );

    // Consume the response body
    let mut res_body_stream = body.into_data_stream();
    let mut size = 0;
    while let Some(chunk) = res_body_stream.next().await {
        match chunk {
            Ok(chunk) => {
                size += chunk.len();
            }
            Err(e) => {
                log::error!("error reading response body: {e:?}");
                break;
            }
        }
    }
    assert!(size > 0);

    Ok(())
}
