use crate::chunk::Chunk;
use crate::network::error::{NetworkError, NetworkResult};
use crate::network::types::{ConnectionConfig, NetworkStats};
use backoff::{backoff::Backoff, ExponentialBackoff};
use bytes::Bytes;
use dashmap::DashMap;
use quinn::{Connection, Endpoint, RecvStream, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<DashMap<String, Connection>>,
    stats: Arc<parking_lot::RwLock<NetworkStats>>,
    /// Whether TLS certificate verification is skipped (INSECURE)
    insecure_mode: bool,
}

impl QuicTransport {
    /// Create new QUIC transport with self-signed certificate
    pub async fn new(config: ConnectionConfig) -> NetworkResult<Self> {
        if config.insecure_skip_verify {
            tracing::warn!(
                "SECURITY WARNING: TLS certificate verification is DISABLED. \
                 This is insecure and should only be used for testing with self-signed certificates. \
                 Set insecure_skip_verify=false and use proper certificates in production."
            );
        }

        let (endpoint, _server_cert) = Self::make_server_endpoint(config.bind_addr)?;

        Ok(Self {
            endpoint,
            connections: Arc::new(DashMap::new()),
            stats: Arc::new(parking_lot::RwLock::new(NetworkStats::default())),
            insecure_mode: config.insecure_skip_verify,
        })
    }

    /// Create server endpoint with self-signed certificate
    fn make_server_endpoint(bind_addr: SocketAddr) -> NetworkResult<(Endpoint, Vec<u8>)> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| NetworkError::CertificateError(e.to_string()))?;
        let cert_der = cert.cert.der().to_vec();
        let priv_key = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der())
            .map_err(|e| NetworkError::CertificateError(e.to_string()))?;

        let mut server_config = ServerConfig::with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(cert_der.clone())],
            priv_key,
        )
        .map_err(|e| NetworkError::CertificateError(e.to_string()))?;

        let transport_config = Arc::get_mut(&mut server_config.transport)
            .ok_or_else(|| NetworkError::QuicError("Failed to get transport config".into()))?;

        transport_config
            .max_concurrent_uni_streams(100_u32.into())
            .max_idle_timeout(Some(Duration::from_secs(60).try_into().unwrap()))
            .keep_alive_interval(Some(Duration::from_secs(5)));

        let endpoint = Endpoint::server(server_config, bind_addr)
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;

        Ok((endpoint, cert_der))
    }

    /// Create client endpoint
    /// If `insecure` is true, accepts any certificate (for testing with self-signed certs)
    /// If `insecure` is false, uses system root certificates for verification
    fn make_client_endpoint(insecure: bool) -> NetworkResult<Endpoint> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?;

        let crypto = if insecure {
            // INSECURE: Skip certificate verification (for testing only)
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
                .with_no_client_auth()
        } else {
            // SECURE: Use system root certificates
            let mut root_store = rustls::RootCertStore::empty();

            // Try to load native/system certificates
            match rustls_native_certs::load_native_certs() {
                Ok(certs) => {
                    for cert in certs {
                        if let Err(e) = root_store.add(cert) {
                            tracing::warn!("Failed to add certificate to root store: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load native certificates: {}. Using webpki roots.",
                        e
                    );
                }
            }

            // Fall back to webpki roots if native certs are empty
            if root_store.is_empty() {
                root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            }

            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };

        let mut client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .map_err(|e| NetworkError::CertificateError(e.to_string()))?,
        ));

        let mut transport_config = quinn::TransportConfig::default();
        transport_config
            .max_concurrent_uni_streams(100_u32.into())
            .max_idle_timeout(Some(Duration::from_secs(60).try_into().unwrap()))
            .keep_alive_interval(Some(Duration::from_secs(5)));

        client_config.transport_config(Arc::new(transport_config));
        endpoint.set_default_client_config(client_config);

        Ok(endpoint)
    }

    /// Connect to remote endpoint
    pub async fn connect(&self, remote_addr: SocketAddr) -> NetworkResult<Connection> {
        let endpoint = Self::make_client_endpoint(self.insecure_mode)?;

        let conn = endpoint
            .connect(remote_addr, "localhost")
            .map_err(|e| NetworkError::ConnectionFailed(e.to_string()))?
            .await?;

        let conn_id = format!("{remote_addr}");
        self.connections.insert(conn_id.clone(), conn.clone());

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.active_connections = self.connections.len();
        }

        Ok(conn)
    }

    /// Accept incoming connection
    pub async fn accept(&self) -> NetworkResult<Connection> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| NetworkError::ConnectionClosed("Endpoint closed".into()))?;

        let conn = incoming.await?;

        let conn_id = format!("{}", conn.remote_address());
        self.connections.insert(conn_id, conn.clone());

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.active_connections = self.connections.len();
        }

        Ok(conn)
    }

    /// Send chunk over QUIC stream
    pub async fn send_chunk(&self, conn: &Connection, chunk: &Chunk) -> NetworkResult<()> {
        let mut send_stream = conn.open_uni().await?;

        // Serialize metadata
        let metadata_bytes = bincode::serialize(&chunk.metadata)?;

        // Send metadata length
        send_stream
            .write_u32(metadata_bytes.len() as u32)
            .await
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        // Send metadata
        send_stream
            .write_all(&metadata_bytes)
            .await
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        // Send data
        send_stream
            .write_all(&chunk.data)
            .await
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        // Finish stream
        send_stream
            .finish()
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;
        send_stream
            .stopped()
            .await
            .map_err(|e| NetworkError::SendFailed(e.to_string()))?;

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_bytes_sent += (metadata_bytes.len() + chunk.data.len()) as u64;
            stats.chunks_sent += 1;
        }

        Ok(())
    }

    /// Receive chunk from QUIC stream
    pub async fn receive_chunk(&self, mut recv_stream: RecvStream) -> NetworkResult<Chunk> {
        // Read metadata length
        let metadata_len = recv_stream
            .read_u32()
            .await
            .map_err(|e| NetworkError::ReceiveFailed(e.to_string()))?
            as usize;

        // Read metadata
        let mut metadata_bytes = vec![0u8; metadata_len];
        recv_stream
            .read_exact(&mut metadata_bytes)
            .await
            .map_err(|e| NetworkError::ReceiveFailed(e.to_string()))?;
        let metadata = bincode::deserialize(&metadata_bytes)?;

        // Read remaining data (max 10MB for safety)
        let data = recv_stream
            .read_to_end(10 * 1024 * 1024)
            .await
            .map_err(|e| NetworkError::ReceiveFailed(e.to_string()))?;

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_bytes_received += (metadata_len + data.len()) as u64;
            stats.chunks_received += 1;
        }

        Ok(Chunk {
            metadata,
            data: Bytes::from(data),
        })
    }

    /// Send chunk with automatic retry using exponential backoff (backoff crate)
    pub async fn send_with_backoff(&self, conn: &Connection, chunk: &Chunk) -> NetworkResult<()> {
        let mut backoff = ExponentialBackoff {
            initial_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
            max_elapsed_time: Some(Duration::from_secs(30)),
            ..Default::default()
        };

        loop {
            match self.send_chunk(conn, chunk).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    match backoff.next_backoff() {
                        Some(duration) => {
                            tracing::warn!("Send failed, retrying in {:?}: {}", duration, e);

                            // Update retry stats
                            {
                                let mut stats = self.stats.write();
                                stats.retransmissions += 1;
                            }

                            tokio::time::sleep(duration).await;
                        }
                        None => {
                            // Max elapsed time exceeded
                            return Err(NetworkError::MaxRetriesExceeded(0));
                        }
                    }
                }
            }
        }
    }

    /// Send chunk with automatic retry
    pub async fn send_with_retry(
        &self,
        conn: &Connection,
        chunk: &Chunk,
        max_retries: u32,
    ) -> NetworkResult<()> {
        let mut attempts = 0;
        let mut backoff = Duration::from_millis(100);

        loop {
            match self.send_chunk(conn, chunk).await {
                Ok(_) => return Ok(()),
                Err(e) if attempts < max_retries => {
                    tracing::warn!(
                        "Send failed (attempt {}/{}): {}",
                        attempts + 1,
                        max_retries,
                        e
                    );

                    // Update retry stats
                    {
                        let mut stats = self.stats.write();
                        stats.retransmissions += 1;
                    }

                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    attempts += 1;
                }
                Err(_e) => {
                    return Err(NetworkError::MaxRetriesExceeded(max_retries));
                }
            }
        }
    }

    /// Get local address
    pub fn local_addr(&self) -> NetworkResult<SocketAddr> {
        self.endpoint.local_addr().map_err(NetworkError::IoError)
    }

    /// Get network statistics
    pub fn stats(&self) -> NetworkStats {
        self.stats.read().clone()
    }

    /// Close all connections
    pub fn close(&self) {
        for entry in self.connections.iter() {
            entry.value().close(0u32.into(), b"closing");
        }
        self.connections.clear();
    }
}

impl Drop for QuicTransport {
    fn drop(&mut self) {
        self.close();
    }
}

// Certificate verifier that accepts any certificate (INSECURE - for testing only)
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{ChunkMetadata, Priority};

    // Initialize crypto provider once for all tests
    fn init_crypto() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    fn create_test_chunk(data: &[u8]) -> Chunk {
        Chunk {
            metadata: ChunkMetadata {
                chunk_id: 1,
                file_id: "test-file".to_string(),
                sequence_number: 0,
                total_chunks: 1,
                data_size: data.len(),
                checksum: [0u8; 32],
                is_parity: false,
                priority: Priority::Normal,
                created_at: chrono::Utc::now().timestamp(),
                file_size: data.len() as u64,
                file_checksum: [0u8; 32],
                data_chunks: 1,
            },
            data: Bytes::from(data.to_vec()),
        }
    }

    #[tokio::test]
    async fn test_create_transport() {
        init_crypto();
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await;
        assert!(transport.is_ok());
    }

    #[tokio::test]
    async fn test_local_addr() {
        init_crypto();
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await.unwrap();
        let addr = transport.local_addr();
        assert!(addr.is_ok());
    }

    #[tokio::test]
    async fn test_send_receive_chunk() {
        init_crypto();
        let config = ConnectionConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        };
        let server = Arc::new(QuicTransport::new(config).await.unwrap());
        let server_addr = server.local_addr().unwrap();

        // Spawn server task
        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let conn = server_clone.accept().await.unwrap();
            let stream = conn.accept_uni().await.unwrap();
            let chunk = server_clone.receive_chunk(stream).await.unwrap();
            assert_eq!(chunk.data, b"test data" as &[u8]);
        });

        // Client sends chunk
        tokio::time::sleep(Duration::from_millis(100)).await;
        let client_config = ConnectionConfig::default();
        let client = QuicTransport::new(client_config).await.unwrap();
        let conn = client.connect(server_addr).await.unwrap();

        let chunk = create_test_chunk(b"test data");
        client.send_chunk(&conn, &chunk).await.unwrap();

        // Wait for server
        tokio::time::timeout(Duration::from_secs(5), server_task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn test_stats() {
        init_crypto();
        let config = ConnectionConfig::default();
        let transport = QuicTransport::new(config).await.unwrap();

        let stats = transport.stats();
        assert_eq!(stats.chunks_sent, 0);
        assert_eq!(stats.chunks_received, 0);
        assert_eq!(stats.total_bytes_sent, 0);
    }

    #[tokio::test]
    async fn test_send_with_retry() {
        init_crypto();
        let config = ConnectionConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        };
        let server = Arc::new(QuicTransport::new(config).await.unwrap());
        let server_addr = server.local_addr().unwrap();

        let server_clone = server.clone();
        tokio::spawn(async move {
            let conn = server_clone.accept().await.unwrap();
            let stream = conn.accept_uni().await.unwrap();
            let _ = server_clone.receive_chunk(stream).await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;
        let client_config = ConnectionConfig::default();
        let client = QuicTransport::new(client_config).await.unwrap();
        let conn = client.connect(server_addr).await.unwrap();

        let chunk = create_test_chunk(b"retry test");
        let result = client.send_with_retry(&conn, &chunk, 3).await;
        assert!(result.is_ok());
    }
}
