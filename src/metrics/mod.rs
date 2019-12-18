use hyper::service::{make_service_fn, service_fn};
use hyper::{header::CONTENT_TYPE, Body, Error, Response, Server};
use prometheus::{Counter, Encoder, Gauge, HistogramVec, TextEncoder};
use std::net::SocketAddr;

pub async fn run_metric_server(addr: SocketAddr) {
    let service = make_service_fn(|_| {
        async {
            Ok::<_, Error>(service_fn(|_req| {
                async {
                    let metric_families = prometheus::gather();
                    let encoder = TextEncoder::new();
                    let mut buffer = vec![];
                    encoder.encode(&metric_families, &mut buffer).unwrap();
                    Ok::<_, Error>(
                        Response::builder()
                            .status(200)
                            .header(CONTENT_TYPE, encoder.format_type())
                            .body(Body::from(buffer))
                            .unwrap(),
                    )
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(service);
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
