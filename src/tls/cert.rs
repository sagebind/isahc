/// An X.509 digital certificate.
#[derive(Clone, Debug)]
pub struct Certificate {
    /// Curl prefers to work in the PEM format, so internally we do as well.
    pem: String,
}

impl Certificate {
    /// Use one or more DER-encoded certificates stored in memory.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn from_der<B: AsRef<[u8]>>(der: B) -> Self {
        let mut base64_spec = data_encoding::BASE64.specification();
        base64_spec.wrap.width = 64;
        base64_spec.wrap.separator.push_str("\r\n");
        let base64 = base64_spec.encoding().unwrap();

        let mut pem = String::new();

        pem.push_str("-----BEGIN CERTIFICATE-----\r\n");
        base64.encode_append(der.as_ref(), &mut pem);
        pem.push_str("-----END CERTIFICATE-----\r\n");

        Self::from_pem(pem)
    }

    /// Use one or more PEM-encoded certificates in the given byte buffer.
    ///
    /// The certificate object takes ownership of the byte buffer. If a borrowed
    /// type is supplied, such as `&[u8]`, then the bytes will be copied.
    ///
    /// The certificates are not parsed or validated here. If a certificate is
    /// malformed or the format is not supported by the underlying SSL/TLS
    /// engine, an error will be returned when attempting to send a request
    /// using the offending certificate.
    pub fn from_pem<B: AsRef<[u8]>>(pem: B) -> Self {
        Self {
            pem: String::from_utf8(pem.as_ref().to_vec()).unwrap(),
        }
    }

    pub(crate) fn as_pem_bytes(&self) -> &[u8] {
        self.pem.as_bytes()
    }

    pub(crate) fn into_pem_string(self) -> String {
        self.pem
    }
}
