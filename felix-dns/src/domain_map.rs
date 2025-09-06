use std::{collections::HashMap, net::Ipv4Addr};

pub struct DomainMap {
    map: HashMap<String, Ipv4Addr>,
}

impl DomainMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn set(&mut self, domain: impl Into<String>, ip: impl Into<Ipv4Addr>) {
        let mut k = domain.into();
        k.make_ascii_lowercase();

        if k.ends_with('.') {
            k.pop();
        }

        self.map.insert(k, ip.into());
    }

    pub fn remove(&mut self, domain: &str) {
        let mut k = domain.to_ascii_lowercase();
        k.make_ascii_lowercase();

        if k.ends_with('.') {
            k.pop();
        }

        self.map.remove(&domain.to_ascii_lowercase());
    }

    pub fn resolve(&self, qname: &str) -> Option<Ipv4Addr> {
        let mut lc = qname.to_ascii_lowercase();

        if lc.ends_with('.') {
            lc.pop();
        }

        if let Some(ip) = self.map.get(&lc) {
            return Some(*ip);
        }

        let labels: Vec<&str> = qname.split('.').collect();
        for i in 0..labels.len().saturating_sub(1) {
            let suffix = labels[i + 1..].join(".");
            let wildcard = format!("*.{}", suffix);

            if let Some(ip) = self.map.get(&wildcard) {
                return Some(*ip);
            }
        }

        None
    }

    pub fn list(&self) -> Vec<(String, Ipv4Addr)> {
        self.map.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }
}
