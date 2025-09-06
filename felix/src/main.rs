use std::net::{SocketAddr, Ipv4Addr};
use felix_dns::{ResolverState, run_udp_server};
use hickory_resolver::{
    config::{NameServerConfig, ResolverConfig}, name_server::GenericConnector, proto::{runtime::TokioRuntimeProvider, xfer::Protocol}, TokioResolver 
};

#[tokio::main]
async fn main() {
    let listen: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let state = ResolverState::new("8.8.8.8:53".parse().unwrap());
    state.add_domain("local.dev", Ipv4Addr::new(127,0,0,1));

    let socket = tokio::net::UdpSocket::bind(listen).await.unwrap();
    let local_addr = socket.local_addr().unwrap();
    drop(socket);

    let handle = run_udp_server(local_addr, state.clone()).await.unwrap();

    let mut cfg = ResolverConfig::new();
    cfg.add_name_server(NameServerConfig {
        socket_addr: local_addr,
        protocol: Protocol::Udp,
        http_endpoint: None,
        tls_dns_name: None,
        trust_negative_responses: true,
        bind_addr: None,
    });

    let provider = GenericConnector::new(TokioRuntimeProvider::new());

    let resolver = TokioResolver::builder_with_config(cfg, provider).build();

    let response = resolver.lookup_ip("local.dev").await.unwrap();
    println!("response: {:?}", response);
    let ips: Vec<Ipv4Addr> = response.iter().filter_map(|ip| {
        match ip {
            std::net::IpAddr::V4(ipv4) => Some(ipv4),
            _ => None,
        }
    }).collect();

    assert!(ips.contains(&Ipv4Addr::new(127,0,0,1)));

    // shutdown server
    handle.shutdown().await;
}
