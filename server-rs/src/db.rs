#![allow(dead_code)]
use std::{str::FromStr, sync::Arc, time::Duration};

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use tokio_postgres::{NoTls, config::SslMode, types::ToSql};

// SECURITY: the certificate is not verified (the equivalent of node-postgres
// `rejectUnauthorized: false`): a pooled database proxy presents a self-signed certificate. The connection is
// still encrypted; it is not protected from an attacker who can sit between the two hosts.
#[derive(Debug)]
struct AcceptAnyCert(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let cause_text = cause.to_string();
        if !text.ends_with(&cause_text) {
            text.push_str(": ");
            text.push_str(&cause_text);
        }
        source = cause.source();
    }
    text
}

const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Db {
    pool: Pool,
}

impl Db {
    pub fn connect(url: &str) -> Result<Self, String> {
        let mut config = tokio_postgres::Config::from_str(url).map_err(|e| e.to_string())?;
        config.connect_timeout(Duration::from_secs(10));
        let manager_config = ManagerConfig { recycling_method: RecyclingMethod::Fast };
        let manager = if config.get_ssl_mode() == SslMode::Disable {
            Manager::from_config(config, NoTls, manager_config)
        } else {
            config.ssl_mode(SslMode::Require);
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let tls = rustls::ClientConfig::builder_with_provider(provider.clone())
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AcceptAnyCert(provider)))
                .with_no_client_auth();
            Manager::from_config(config, tokio_postgres_rustls::MakeRustlsConnect::new(tls), manager_config)
        };
        let max = std::env::var("PG_POOL_MAX").ok().and_then(|v| v.parse().ok()).filter(|n| *n >= 1).unwrap_or(3);
        let pool = Pool::builder(manager).max_size(max).build().map_err(|e| e.to_string())?;
        Ok(Self { pool })
    }

    pub async fn text(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Option<String>, String> {
        let work = async {
            let client = self.pool.get().await.map_err(|e| chain(&e))?;
            let row = client.query_opt(sql, params).await.map_err(|e| e.to_string())?;
            Ok(row.map(|r| r.get::<_, String>(0)))
        };
        tokio::time::timeout(QUERY_TIMEOUT, work).await.map_err(|_| "Query read timeout".to_string())?
    }

    /// Rows affected.
    pub async fn execute(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, String> {
        let work = async {
            let client = self.pool.get().await.map_err(|e| chain(&e))?;
            client.execute(sql, params).await.map_err(|e| e.to_string())
        };
        tokio::time::timeout(QUERY_TIMEOUT, work).await.map_err(|_| "Query read timeout".to_string())?
    }

    /// Every row's first column, which the query must produce as text.
    pub async fn texts(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<String>, String> {
        let work = async {
            let client = self.pool.get().await.map_err(|e| chain(&e))?;
            let rows = client.query(sql, params).await.map_err(|e| e.to_string())?;
            Ok(rows.iter().map(|r| r.get::<_, String>(0)).collect())
        };
        tokio::time::timeout(QUERY_TIMEOUT, work).await.map_err(|_| "Query read timeout".to_string())?
    }

    pub async fn batch(&self, sql: &str) -> Result<(), String> {
        let client = self.pool.get().await.map_err(|e| chain(&e))?;
        client.batch_execute(sql).await.map_err(|e| e.to_string())
    }

    pub async fn client(&self) -> Result<deadpool_postgres::Object, String> {
        self.pool.get().await.map_err(|e| chain(&e))
    }

    pub fn stats(&self) -> serde_json::Value {
        let s = self.pool.status();
        serde_json::json!({ "total": s.size, "idle": s.available, "waiting": s.waiting })
    }
}
