//! Rustls cryptographic provider initialization for the Weaver CLI.

#[cfg(all(not(any(test, doc)), not(clippy)))]
const _: () = {
    const RING: u8 = if cfg!(feature = "crypto-ring") { 1 } else { 0 };
    const AWS_LC: u8 = if cfg!(feature = "crypto-aws-lc") {
        1
    } else {
        0
    };
    const OPENSSL: u8 = if cfg!(feature = "crypto-openssl") {
        1
    } else {
        0
    };
    const SYMCRYPT: u8 = if cfg!(feature = "crypto-symcrypt") {
        1
    } else {
        0
    };

    if RING + AWS_LC + OPENSSL + SYMCRYPT > 1 {
        panic!(
            "Crypto provider features are mutually exclusive. Enable exactly one of: \
             crypto-ring, crypto-aws-lc, crypto-openssl, or crypto-symcrypt. Use \
             --no-default-features before selecting a provider other than crypto-ring."
        );
    }
};

#[cfg(all(
    feature = "crypto-symcrypt",
    not(any(target_os = "linux", target_os = "windows")),
    not(any(test, doc)),
    not(clippy)
))]
compile_error!(
    "Feature `crypto-symcrypt` is only supported on Linux and Windows. Select another provider, \
     such as `crypto-ring`, on this platform."
);

#[cfg(any(
    feature = "crypto-ring",
    feature = "crypto-aws-lc",
    feature = "crypto-openssl",
    all(
        feature = "crypto-symcrypt",
        any(target_os = "linux", target_os = "windows")
    )
))]
fn selected_crypto_provider() -> (rustls::crypto::CryptoProvider, &'static str) {
    #[cfg(feature = "crypto-ring")]
    return (rustls::crypto::ring::default_provider(), "ring");

    #[cfg(all(not(feature = "crypto-ring"), feature = "crypto-aws-lc"))]
    return (rustls::crypto::aws_lc_rs::default_provider(), "aws-lc-rs");

    #[cfg(all(
        not(feature = "crypto-ring"),
        not(feature = "crypto-aws-lc"),
        feature = "crypto-openssl"
    ))]
    return (rustls_openssl::default_provider(), "OpenSSL");

    #[cfg(all(
        not(feature = "crypto-ring"),
        not(feature = "crypto-aws-lc"),
        not(feature = "crypto-openssl"),
        feature = "crypto-symcrypt",
        any(target_os = "linux", target_os = "windows")
    ))]
    return (rustls_symcrypt::default_symcrypt_provider(), "SymCrypt");
}

/// Installs the provider selected by the CLI's `crypto-*` feature.
///
/// A build without a provider remains valid for consumers that only need
/// plaintext transports. TLS operations in such a build fail at runtime.
pub(crate) fn install_crypto_provider() -> Result<(), String> {
    #[cfg(any(
        feature = "crypto-ring",
        feature = "crypto-aws-lc",
        feature = "crypto-openssl",
        all(
            feature = "crypto-symcrypt",
            any(target_os = "linux", target_os = "windows")
        )
    ))]
    {
        let (provider, name) = selected_crypto_provider();
        provider
            .install_default()
            .map_err(|_| format!("Failed to install the {name} Rustls crypto provider"))
    }

    #[cfg(not(any(
        feature = "crypto-ring",
        feature = "crypto-aws-lc",
        feature = "crypto-openssl",
        all(
            feature = "crypto-symcrypt",
            any(target_os = "linux", target_os = "windows")
        )
    )))]
    {
        Ok(())
    }
}
