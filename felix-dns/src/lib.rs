pub mod domain_map;
pub mod resolver_state;
pub mod server_handler;
pub mod sqlite_domain_store;

pub use domain_map::DomainMap;
pub use resolver_state::ResolverState;
pub use server_handler::run_udp_server;
pub use sqlite_domain_store::SqliteDomainStore;


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

    #[tokio::test]
    async fn test_sqlite_domain_store() {
        // Sử dụng in-memory SQLite database cho tests
        let store = SqliteDomainStore::new(":memory:").await.unwrap();
        
        // Test set và resolve
        store.set("example.com", Ipv4Addr::new(192, 168, 1, 1)).await.unwrap();
        let result = store.resolve("example.com").await.unwrap();
        assert_eq!(result, Some(Ipv4Addr::new(192, 168, 1, 1)));
        
        // Test wildcard
        store.set("*.test.dev", Ipv4Addr::new(10, 0, 0, 1)).await.unwrap();
        let result = store.resolve("api.test.dev").await.unwrap();
        assert_eq!(result, Some(Ipv4Addr::new(10, 0, 0, 1)));
        
        // Test list
        let domains = store.list().await.unwrap();
        assert_eq!(domains.len(), 2);
        
        // Test remove
        store.remove("example.com").await.unwrap();
        let result = store.resolve("example.com").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_resolver_state_with_sqlite() {
        // Sử dụng in-memory SQLite database cho tests
        let state = ResolverState::new_with_sqlite("8.8.8.8:53".parse().unwrap(), ":memory:").await.unwrap();
        
        // Test add và resolve
        state.add_domain("test.local", Ipv4Addr::new(127, 0, 0, 1)).await.unwrap();
        let result = state.resolve("test.local").await.unwrap();
        assert_eq!(result, Some(Ipv4Addr::new(127, 0, 0, 1)));
        
        // Test list
        let domains = state.list_domains().await.unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0], ("test.local".to_string(), Ipv4Addr::new(127, 0, 0, 1)));
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
            state.add_domain_sync("local.dev", Ipv4Addr::new(127,0,0,1));

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
