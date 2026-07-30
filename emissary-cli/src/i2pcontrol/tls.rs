// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rcgen::{CertificateParams, KeyPair};
use rustls_pemfile::Item;
use tokio_rustls::rustls::crypto::ring;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tracing;

use super::errors::I2pControlError;

const LOG_TARGET: &str = "emissary::i2pcontrol::tls";

/// Managed certificate directory name under the base path.
const MANAGED_CERT_DIR: &str = "i2pcontrol-certs";

/// Managed certificate filename.
const CERT_FILE: &str = "cert.pem";

/// Managed private key filename.
const KEY_FILE: &str = "key.pem";

/// TLS configuration for I2PControl.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Optional explicit certificate path.
    pub certificate: Option<PathBuf>,
    /// Optional explicit private key path.
    pub private_key: Option<PathBuf>,
}

impl TlsConfig {
    /// Returns true if this config provides explicit TLS material paths.
    pub fn is_explicit(&self) -> bool {
        self.certificate.is_some() || self.private_key.is_some()
    }
}

/// Load or generate TLS material and build a `ServerConfig`.
///
/// If explicit paths are provided, loads from those paths.
/// Otherwise, generates or reuses a managed self-signed certificate under `base_path`.
pub fn build_tls_config(
    tls: &TlsConfig,
    base_path: &Path,
) -> Result<Arc<ServerConfig>, I2pControlError> {
    let (certs, key) = if tls.is_explicit() {
        load_explicit_tls(tls)?
    } else {
        load_or_generate_managed_tls(base_path)?
    };

    let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| I2pControlError::Tls(format!("TLS config error: {e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| I2pControlError::Tls(format!("TLS cert/key error: {e}")))?;

    Ok(Arc::new(config))
}

/// Load TLS material from explicit operator-provided paths.
fn load_explicit_tls(
    tls: &TlsConfig,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), I2pControlError> {
    let cert_path = tls
        .certificate
        .as_ref()
        .ok_or_else(|| I2pControlError::Tls("Certificate path required".into()))?;
    let key_path = tls
        .private_key
        .as_ref()
        .ok_or_else(|| I2pControlError::Tls("Private key path required".into()))?;

    let certs = load_certs(cert_path)?;
    let key = load_key(key_path)?;

    Ok((certs, key))
}

/// Load or generate a managed self-signed certificate.
pub fn load_or_generate_managed_tls(
    base_path: &Path,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), I2pControlError> {
    let cert_dir = base_path.join(MANAGED_CERT_DIR);
    let cert_path = cert_dir.join(CERT_FILE);
    let key_path = cert_dir.join(KEY_FILE);

    // Try to load existing certificate material
    if cert_path.exists() && key_path.exists() {
        match load_certs(&cert_path).and_then(|certs| load_key(&key_path).map(|key| (certs, key))) {
            Ok(result) => {
                tracing::info!(
                    target: LOG_TARGET,
                    "loaded existing managed TLS certificate",
                );
                return Ok(result);
            }
            Err(e) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?e,
                    "existing TLS material invalid, regenerating",
                );
            }
        }
    }

    // Generate new self-signed certificate
    generate_managed_tls(&cert_dir, &cert_path, &key_path)
}

/// Generate a new self-signed certificate and save it.
fn generate_managed_tls(
    cert_dir: &Path,
    cert_path: &Path,
    key_path: &Path,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), I2pControlError> {
    // Create directory if it doesn't exist
    fs::create_dir_all(cert_dir)
        .map_err(|e| I2pControlError::Tls(format!("Failed to create cert directory: {e}")))?;

    let key_pair = KeyPair::generate()
        .map_err(|e| I2pControlError::Tls(format!("Failed to generate key pair: {e}")))?;

    let mut params = CertificateParams::new(vec!["localhost".to_string()])
        .map_err(|e| I2pControlError::Tls(format!("Failed to create cert params: {e}")))?;

    params.distinguished_name.push(rcgen::DnType::CommonName, "Emissary I2PControl");

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| I2pControlError::Tls(format!("Failed to sign certificate: {e}")))?;

    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(key_pair.serialize_der())
        .map_err(|e| I2pControlError::Tls(format!("Failed to serialize key: {e}")))?;

    // Write certificate as DER
    fs::write(cert_path, cert.der())
        .map_err(|e| I2pControlError::Tls(format!("Failed to write certificate: {e}")))?;

    // Write private key as DER
    fs::write(key_path, key_pair.serialize_der())
        .map_err(|e| I2pControlError::Tls(format!("Failed to write private key: {e}")))?;

    tracing::info!(
        target: LOG_TARGET,
        ?cert_path,
        "generated managed self-signed TLS certificate",
    );

    Ok((vec![cert_der], key_der))
}

/// Load certificates from a file (tries DER first, then PEM).
fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>, I2pControlError> {
    let data = fs::read(path)
        .map_err(|e| I2pControlError::Tls(format!("Failed to open certificate file: {e}")))?;

    // Try PEM first (has header)
    if data.starts_with(b"-----") {
        let mut reader = BufReader::new(data.as_slice());
        let mut certs = Vec::new();
        for item in rustls_pemfile::read_all(&mut reader).flatten() {
            if let Item::X509Certificate(cert) = item {
                certs.push(cert);
            }
        }
        if !certs.is_empty() {
            return Ok(certs);
        }
    }

    // Default: treat as DER
    let cert = CertificateDer::from(data);
    Ok(vec![cert])
}

/// Load a private key from a file (tries DER first, then PEM).
fn load_key(path: &Path) -> Result<PrivateKeyDer<'static>, I2pControlError> {
    let data = fs::read(path)
        .map_err(|e| I2pControlError::Tls(format!("Failed to open private key file: {e}")))?;

    // Try PEM first (has header)
    if data.starts_with(b"-----") {
        let mut reader = BufReader::new(data.as_slice());
        for item in rustls_pemfile::read_all(&mut reader).flatten() {
            match item {
                Item::Pkcs1Key(k) => return Ok(PrivateKeyDer::Pkcs1(k)),
                Item::Sec1Key(k) => return Ok(PrivateKeyDer::Sec1(k)),
                Item::Pkcs8Key(k) => return Ok(PrivateKeyDer::Pkcs8(k)),
                _ => {}
            }
        }
    }

    // Default: treat as PKCS8 DER (rcgen generates PKCS8)
    Ok(PrivateKeyDer::Pkcs8(data.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn managed_tls_generates_and_loads() {
        let dir = tempdir().unwrap();
        let base = dir.path();

        // First call generates
        let (certs1, _) = load_or_generate_managed_tls(base).unwrap();
        assert!(!certs1.is_empty());

        // Second call loads same material
        let (certs2, _) = load_or_generate_managed_tls(base).unwrap();
        assert_eq!(certs1[0].as_ref(), certs2[0].as_ref());
    }

    #[test]
    fn managed_tls_recovers_from_invalid_cert() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let cert_dir = base.join(MANAGED_CERT_DIR);
        fs::create_dir_all(&cert_dir).unwrap();

        // Write invalid cert
        fs::write(cert_dir.join(CERT_FILE), "not a cert").unwrap();
        fs::write(cert_dir.join(KEY_FILE), "not a key").unwrap();

        // Should regenerate
        let result = load_or_generate_managed_tls(base);
        assert!(result.is_ok());
    }

    #[test]
    fn tls_config_is_explicit() {
        let c1 = TlsConfig {
            certificate: None,
            private_key: None,
        };
        assert!(!c1.is_explicit());

        let c2 = TlsConfig {
            certificate: Some(PathBuf::from("/cert")),
            private_key: None,
        };
        assert!(c2.is_explicit());
    }
}
