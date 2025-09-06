use std::{net::{Ipv4Addr, SocketAddr}, sync::Arc};

use parking_lot::RwLock;

use crate::domain_map::DomainMap;

#[derive(Clone)]
pub struct ResolverState {
    enabled: Arc<RwLock<bool>>,
    domain_map: Arc<RwLock<DomainMap>>,
    upstream: Arc<RwLock<SocketAddr>>,
}

impl ResolverState {
    pub fn new(upstream: SocketAddr) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(true)),
            domain_map: Arc::new(RwLock::new(DomainMap::new())),
            upstream: Arc::new(RwLock::new(upstream)),
        }
    }

    pub fn set_enabled(&self, v: bool) {
        *self.enabled.write() = v;
    }

    pub fn enabled(&self) -> bool {
        *self.enabled.read()
    }

    pub fn set_upstream(&self, addr: SocketAddr) {
        *self.upstream.write() = addr;
    }

    pub fn upstream(&self) -> SocketAddr {
        *self.upstream.read()
    }

    pub fn add_domain(&self, domain: &str, ip: Ipv4Addr) {
        self.domain_map.write().set(domain.to_string(), ip);
    }

    pub fn remove_domain(&self, domain: &str) {
        self.domain_map.write().remove(domain);
    }

    pub fn list_domains(&self) -> Vec<(String, Ipv4Addr)> {
        self.domain_map.read().list()
    }

    pub fn resolve(&self, qname: &str) -> Option<Ipv4Addr> {
        println!("Resolving {} in domain map", qname);
        self.domain_map.read().resolve(qname)
    }
}
