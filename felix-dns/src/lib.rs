pub mod domain_map;
pub mod resolver_state;
pub mod server_handler;

pub use domain_map::DomainMap;
pub use resolver_state::ResolverState;
pub use server_handler::run_udp_server;


#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_exact_and_wildcard() {
        let mut dm = DomainMap::new();
        dm.set("foo.dev", Ipv4Addr::new(127, 0, 0, 1));
        dm.set("*.example.com", Ipv4Addr::new(10, 0, 0, 42));

        // exact match
        assert_eq!(dm.resolve("foo.dev"), Some(Ipv4Addr::new(127, 0, 0, 1)));

        // wildcard match
        assert_eq!(dm.resolve("api.example.com"), Some(Ipv4Addr::new(10, 0, 0, 42)));
        assert_eq!(dm.resolve("deep.sub.example.com"), Some(Ipv4Addr::new(10, 0, 0, 42)));

        // not found
        assert_eq!(dm.resolve("unknown.test"), None);
    }

    #[test]
    fn test_remove() {
        let mut dm = DomainMap::new();
        dm.set("foo.dev", Ipv4Addr::new(127, 0, 0, 1));
        assert!(dm.resolve("foo.dev").is_some());

        dm.remove("foo.dev");
        assert!(dm.resolve("foo.dev").is_none());
    }

    #[test]
    fn test_list() {
        let mut dm = DomainMap::new();
        dm.set("foo.dev", Ipv4Addr::new(127, 0, 0, 1));
        assert!(dm.resolve("foo.dev").is_some());

        assert_eq!(dm.list(), vec![("foo.dev".to_string(), Ipv4Addr::new(127, 0, 0, 1))]);
    }

    #[test]
    fn test_set_and_resolve() {
        let mut dm = DomainMap::new();
        dm.set("foo.dev", Ipv4Addr::new(127, 0, 0, 1));
        assert!(dm.resolve("foo.dev").is_some());
    }

    #[test]
    fn test_set_and_resolve_wildcard() {
        let mut dm = DomainMap::new();
        dm.set("*.dev", Ipv4Addr::new(127, 0, 0, 1));
        assert!(dm.resolve("foo.dev").is_some());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::runtime::Runtime;
    use std::net::{SocketAddr, Ipv4Addr};
    use hickory_resolver::{
        config::{NameServerConfig, ResolverConfig}, name_server::GenericConnector, proto::{runtime::TokioRuntimeProvider, xfer::Protocol}, TokioResolver 
    };

    #[test]
    fn test_server_resolves_custom_domain() {
        let rt = Runtime::new().unwrap();
        println!("Testing server_resolves_custom_domain");
        rt.block_on(async {
            // start DNS server
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
        });
    }
}
