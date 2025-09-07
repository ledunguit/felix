use anyhow::Result;
use sqlx::{Pool, Sqlite, SqlitePool};
use std::net::Ipv4Addr;

#[derive(Clone)]
pub struct SqliteDomainStore {
    pool: Pool<Sqlite>,
}

impl SqliteDomainStore {
    pub async fn new(database_path: &str) -> Result<Self> {
        let connection_string = if database_path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite:{}?mode=rwc", database_path)
        };
        let pool = SqlitePool::connect(&connection_string).await?;

        let store = Self { pool };
        store.initialize_schema().await?;

        Ok(store)
    }

    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS domain_mappings (
                domain TEXT PRIMARY KEY,
                ip_a INTEGER NOT NULL,
                ip_b INTEGER NOT NULL, 
                ip_c INTEGER NOT NULL,
                ip_d INTEGER NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s', 'now')),
                updated_at INTEGER DEFAULT (strftime('%s', 'now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        let query = r"CREATE TRIGGER IF NOT EXISTS update_domain_mappings_timestamp
                AFTER UPDATE ON domain_mappings
                BEGIN
                    UPDATE domain_mappings SET updated_at = strftime('%s', 'now') WHERE domain = NEW.domain;
                END";
        sqlx::query(query).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn set(&self, domain: &str, ip: Ipv4Addr) -> Result<()> {
        let mut normalized_domain = domain.to_ascii_lowercase();
        if normalized_domain.ends_with('.') {
            normalized_domain.pop();
        }

        let octets = ip.octets();

        sqlx::query(
            "INSERT OR REPLACE INTO domain_mappings (domain, ip_a, ip_b, ip_c, ip_d) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&normalized_domain)
        .bind(octets[0] as i32)
        .bind(octets[1] as i32)
        .bind(octets[2] as i32)
        .bind(octets[3] as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove(&self, domain: &str) -> Result<()> {
        let mut normalized_domain = domain.to_ascii_lowercase();
        if normalized_domain.ends_with('.') {
            normalized_domain.pop();
        }

        sqlx::query("DELETE FROM domain_mappings WHERE domain = ?")
            .bind(&normalized_domain)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn resolve(&self, qname: &str) -> Result<Option<Ipv4Addr>> {
        let mut normalized_qname = qname.to_ascii_lowercase();
        if normalized_qname.ends_with('.') {
            normalized_qname.pop();
        }

        if let Some(ip) = self.get_exact_match(&normalized_qname).await? {
            return Ok(Some(ip));
        }

        let labels: Vec<&str> = normalized_qname.split('.').collect();
        for i in 0..labels.len().saturating_sub(1) {
            let suffix = labels[i + 1..].join(".");
            let wildcard = format!("*.{}", suffix);

            if let Some(ip) = self.get_exact_match(&wildcard).await? {
                return Ok(Some(ip));
            }
        }

        Ok(None)
    }

    async fn get_exact_match(&self, domain: &str) -> Result<Option<Ipv4Addr>> {
        let row = sqlx::query_as::<_, (i32, i32, i32, i32)>(
            "SELECT ip_a, ip_b, ip_c, ip_d FROM domain_mappings WHERE domain = ?",
        )
        .bind(domain)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((ip_a, ip_b, ip_c, ip_d)) = row {
            let ip = Ipv4Addr::new(ip_a as u8, ip_b as u8, ip_c as u8, ip_d as u8);
            Ok(Some(ip))
        } else {
            Ok(None)
        }
    }

    pub async fn list(&self) -> Result<Vec<(String, Ipv4Addr)>> {
        let rows = sqlx::query_as::<_, (String, i32, i32, i32, i32)>(
            "SELECT domain, ip_a, ip_b, ip_c, ip_d FROM domain_mappings ORDER BY domain",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for (domain, ip_a, ip_b, ip_c, ip_d) in rows {
            let ip = Ipv4Addr::new(ip_a as u8, ip_b as u8, ip_c as u8, ip_d as u8);
            result.push((domain, ip));
        }

        Ok(result)
    }

    pub async fn count(&self) -> Result<i64> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM domain_mappings")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    pub async fn clear(&self) -> Result<()> {
        sqlx::query("DELETE FROM domain_mappings")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
