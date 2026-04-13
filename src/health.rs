//! Tiny HTTP server exposing `/healthz` and `/readyz`.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tracing::{info, warn};

pub async fn serve(
    addr: SocketAddr,
    ready: Arc<AtomicBool>,
    cancel: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "health server listening");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("health server shutting down");
                return Ok(());
            }
            accept = listener.accept() => {
                let (stream, _) = match accept {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(error = %e, "accept error");
                        continue;
                    }
                };
                let io = TokioIo::new(stream);
                let ready = ready.clone();
                tokio::spawn(async move {
                    let svc = service_fn(move |req| handle(req, ready.clone()));
                    if let Err(e) = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, svc)
                        .await
                    {
                        warn!(error = %e, "http1 serve error");
                    }
                });
            }
        }
    }
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    ready: Arc<AtomicBool>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let resp = match req.uri().path() {
        "/healthz" => text(StatusCode::OK, "ok"),
        "/readyz" => {
            if ready.load(Ordering::SeqCst) {
                text(StatusCode::OK, "ready")
            } else {
                text(StatusCode::SERVICE_UNAVAILABLE, "starting")
            }
        }
        _ => text(StatusCode::NOT_FOUND, "not found"),
    };
    Ok(resp)
}

fn text(code: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(code)
        .header("content-type", "text/plain")
        .body(Full::new(Bytes::from_static(body.as_bytes())))
        .unwrap()
}
