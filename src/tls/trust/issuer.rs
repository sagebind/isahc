use super::super::PathOrBlob;
use crate::{
    blob::Blob,
    config::setopt::{EasyHandle, SetOpt, SetOptError, SetOptProxy},
    handler::BlobOptions,
};
use curl_sys::{CURLOPT_ISSUERCERT_BLOB, CURLOPT_PROXY_ISSUERCERT_BLOB};
use std::path::PathBuf;

/// A specific issuer certificate to use when validating a server's certificate
/// chain.
///
/// Note that curl only supports specifying issuer certificates in PEM format
/// under the hood, so if your certificate is in DER format, you will need to
/// convert it to PEM first.
#[derive(Clone, Debug)]
pub struct Issuer {
    pem: PathOrBlob,
}

impl Issuer {
    /// Validate the issuer against a PEM-encoded certificate.
    ///
    /// The certificate can be supplied as any type which can be referenced as
    /// contiguous bytes. This could be a simple [`String`], or a pre-parsed PEM
    /// object that allows access to its underlying bytes. This takes ownership
    /// of the bytes regardless.
    ///
    /// The certificate is not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn from_pem<B: AsRef<[u8]> + Send + Sync + 'static>(pem: B) -> Self {
        Self {
            pem: PathOrBlob::Blob(Blob::new(pem)),
        }
    }

    /// Validate the issuer against a DER-encoded certificate.
    ///
    /// The certificate is copied and first converted into PEM format before
    /// being stored.
    ///
    /// The certificate is not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn copy_from_der<B: AsRef<[u8]>>(der: B) -> Self {
        let pem =
            pem_rfc7468::encode_string("CERTIFICATE", Default::default(), der.as_ref()).unwrap();

        Self::from_pem(pem)
    }

    /// Validate the issuer against a PEM-encoded certificate stored in a file
    /// with the given path.
    ///
    /// The certificate is not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn from_pem_file<P>(path: P) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            pem: PathOrBlob::Path(path.into()),
        }
    }
}

impl SetOpt for Issuer {
    fn set_opt(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        match &self.pem {
            PathOrBlob::Path(path) => easy.issuer_cert(path)?,
            PathOrBlob::Blob(blob) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_ISSUERCERT_BLOB, blob)?;
            },
        }

        Ok(())
    }
}

impl SetOptProxy for Issuer {
    fn set_opt_proxy(&self, easy: &mut EasyHandle) -> Result<(), SetOptError> {
        match &self.pem {
            PathOrBlob::Path(path) => easy.proxy_issuer_cert(path)?,
            PathOrBlob::Blob(blob) => unsafe {
                easy.setopt_blob_nocopy(CURLOPT_PROXY_ISSUERCERT_BLOB, blob)?;
            },
        }

        Ok(())
    }
}
