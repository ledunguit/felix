use std::{net::{Ipv4Addr, SocketAddr}, sync::Arc};

use parking_lot::RwLock;
use anyhow::Result;

use crate::{domain_map::DomainMap, sqlite_domain_store::SqliteDomainStore};

#[derive(Clone)]
pub enum DomainStorage {
    InMemory(Arc<RwLock<DomainMap>>),
    Sqlite(SqliteDomainStore),
}

#[derive(Clone)]
pub struct ResolverState {
    enabled: Arc<RwLock<bool>>,
    storage: DomainStorage,
    upstream: Arc<RwLock<SocketAddr>>,
}

impl ResolverState {
    pub fn new(upstream: SocketAddr) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(true)),
            storage: DomainStorage::InMemory(Arc::new(RwLock::new(DomainMap::new()))),
            upstream: Arc::new(RwLock::new(upstream)),
        }
    }
    
    pub async fn new_with_sqlite(upstream: SocketAddr, database_path: &str) -> Result<Self> {
        let sqlite_store = SqliteDomainStore::new(database_path).await?;
        Ok(Self {
            enabled: Arc::new(RwLock::new(true)),
            storage: DomainStorage::Sqlite(sqlite_store),
            upstream: Arc::new(RwLock::new(upstream)),
        })
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

    pub async fn add_domain(&self, domain: &str, ip: Ipv4Addr) -> Result<()> {
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                domain_map.write().set(domain.to_string(), ip);
                Ok(())
            }
            DomainStorage::Sqlite(store) => {
                store.set(domain, ip).await
            }
        }
    }
    
    pub fn add_domain_sync(&self, domain: &str, ip: Ipv4Addr) {
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                domain_map.write().set(domain.to_string(), ip);
            }
            DomainStorage::Sqlite(_) => {
                log::warn!("add_domain_sync called with SQLite storage - use add_domain instead");
            }
        }
    }

    pub async fn remove_domain(&self, domain: &str) -> Result<()> {
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                domain_map.write().remove(domain);
                Ok(())
            }
            DomainStorage::Sqlite(store) => {
                store.remove(domain).await
            }
        }
    }

    pub async fn list_domains(&self) -> Result<Vec<(String, Ipv4Addr)>> {
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                Ok(domain_map.read().list())
            }
            DomainStorage::Sqlite(store) => {
                store.list().await
            }
        }
    }

    pub async fn resolve(&self, qname: &str) -> Result<Option<Ipv4Addr>> {
        println!("Resolving {} in domain map", qname);
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                Ok(domain_map.read().resolve(qname))
            }
            DomainStorage::Sqlite(store) => {
                store.resolve(qname).await
            }
        }
    }
    
    pub fn resolve_sync(&self, qname: &str) -> Option<Ipv4Addr> {
        println!("Resolving {} in domain map", qname);
        match &self.storage {
            DomainStorage::InMemory(domain_map) => {
                domain_map.read().resolve(qname)
            }
            DomainStorage::Sqlite(_) => {
                log::warn!("resolve_sync called with SQLite storage - use resolve instead");
                None
            }
        }
    }
}
