use lazy_static::lazy_static;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, error, info};

lazy_static! {
    static ref ROOT_CA_CACHE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

#[derive(Debug, thiserror::Error)]
pub enum RootCertificateError {
    #[error("Failed to get certificate: {0}")]
    CertificateError(String),

    #[error("Cache lock error: {0}")]
    CacheLockError(String),
}

pub async fn get_root_ca(domain: &str, port: u16) -> Result<String, RootCertificateError> {
    use std::time::Duration;
    use tokio::time::timeout;

    // Define a 5-second timeout for the entire operation
    let result = timeout(Duration::from_secs(5), async {
        // Check cache first
        if let Some(cert) = ROOT_CA_CACHE
            .lock()
            .map_err(|e| RootCertificateError::CacheLockError(e.to_string()))?
            .get(domain)
        {
            debug!("Using cached root CA certificate for {}", domain);
            return Ok(cert.clone());
        }

        info!("Fetching root CA certificate for {}", domain);

        // Create OpenSSL connector with default verification settings
        let mut connector = SslConnector::builder(SslMethod::tls())
            .map_err(|e| RootCertificateError::CertificateError(e.to_string()))?;

        // Use default verification settings which validate the certificate chain
        connector.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

        // Load system root certificates
        connector.set_default_verify_paths().map_err(|e| {
            RootCertificateError::CertificateError(format!(
                "Failed to load system root certificates: {}",
                e
            ))
        })?;

        // Build the connector
        let connector = connector.build();

        // Use std::net::TcpStream instead of Tokio's TcpStream
        let stream = std::net::TcpStream::connect(format!("{}:{}", domain, port)).map_err(|e| {
            RootCertificateError::CertificateError(format!("TCP connection failed: {}", e))
        })?;

        // Configure SSL with hostname verification enabled
        let ssl = connector
            .configure()
            .map_err(|e| RootCertificateError::CertificateError(e.to_string()))?
            .verify_hostname(true)
            .into_ssl(domain)
            .map_err(|e| RootCertificateError::CertificateError(e.to_string()))?;

        let mut ssl_stream = ssl.connect(stream).map_err(|e| {
            RootCertificateError::CertificateError(format!(
                "SSL connection failed (certificate may not be trusted): {}",
                e
            ))
        })?;

        // Get the full certificate chain
        let cert_chain = ssl_stream.ssl().peer_cert_chain().ok_or_else(|| {
            RootCertificateError::CertificateError("No certificate chain found".into())
        })?;

        // Get the root certificate (last in the chain)
        let root_cert = cert_chain.into_iter().last().ok_or_else(|| {
            RootCertificateError::CertificateError("No root certificate found".into())
        })?;

        // Convert the certificate to PEM format
        let cert_pem = root_cert.to_pem().map_err(|e| {
            RootCertificateError::CertificateError(format!("PEM conversion failed: {}", e))
        })?;

        let cert_pem = String::from_utf8(cert_pem).map_err(|e| {
            RootCertificateError::CertificateError(format!("UTF-8 conversion failed: {}", e))
        })?;

        // Cache the certificate
        ROOT_CA_CACHE
            .lock()
            .map_err(|e| RootCertificateError::CacheLockError(e.to_string()))?
            .insert(domain.to_string(), cert_pem.clone());

        // Explicitly shutdown the connection
        ssl_stream.shutdown().map_err(|e| {
            RootCertificateError::CertificateError(format!(
                "Failed to shutdown SSL connection: {}",
                e
            ))
        })?;

        Ok(cert_pem)
    })
    .await;

    match result {
        Ok(Ok(cert)) => Ok(cert),
        Ok(Err(e)) => {
            error!("Error occurred: {}", e);
            Err(e)
        }
        Err(_) => {
            error!("Operation timed out");
            Err(RootCertificateError::CertificateError(
                "Operation timed out".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_root_ca() {
        let result = get_root_ca("example.com", 443).await;
        assert!(result.is_ok(), "Failed to get root CA: {:?}", result.err());

        let cert = result.unwrap();
        assert!(!cert.is_empty(), "Root CA certificate should not be empty");
        assert!(
            cert.contains("BEGIN CERTIFICATE"),
            "Certificate should be in PEM format"
        );
        assert_eq!(
            cert.matches("BEGIN CERTIFICATE").count(),
            1,
            "Certificate should only have one BEGIN header"
        );
    }
}
