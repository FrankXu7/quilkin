/*
 * Copyright 2020 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use quilkin::{endpoint::Endpoint, test_utils::TestHelper};

#[tokio::test]
async fn metrics_server() {
    let mut t = TestHelper::default();

    // create an echo server as an endpoint.
    let echo = t.run_echo_server().await;

    // create server configuration
    let server_addr = quilkin::test_utils::available_addr().await;
    let server_proxy = quilkin::cli::Proxy {
        port: server_addr.port(),
        ..<_>::default()
    };
    let server_config = std::sync::Arc::new(quilkin::Config::default());
    server_config
        .clusters
        .modify(|clusters| clusters.insert_default(vec![Endpoint::new(echo.clone())]));
    t.run_server(
        server_config,
        server_proxy,
        Some(Some("[::]:9092".parse().unwrap())),
    );

    // create a local client
    let client_port = 12347;
    let client_proxy = quilkin::cli::Proxy {
        port: client_port,
        ..<_>::default()
    };
    let client_config = std::sync::Arc::new(quilkin::Config::default());
    client_config
        .clusters
        .modify(|clusters| clusters.insert_default(vec![Endpoint::new(server_addr.into())]));
    t.run_server(client_config, client_proxy, None);

    // let's send the packet
    let (mut recv_chan, socket) = t.open_socket_and_recv_multiple_packets().await;

    // game_client
    let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), client_port);
    tracing::info!(address = %local_addr, "Sending hello");
    socket.send_to(b"hello", &local_addr).await.unwrap();

    let _ = recv_chan.recv().await.unwrap();
    let client = hyper::Client::new();

    let resp = client
        .get(hyper::Uri::from_static("http://localhost:9092/metrics"))
        .await
        .map(|resp| resp.into_body())
        .map(hyper::body::to_bytes)
        .unwrap()
        .await
        .unwrap();

    let response = String::from_utf8(resp.to_vec()).unwrap();
    assert!(response.contains(r#"quilkin_packets_total{event="read"} 2"#));
}
