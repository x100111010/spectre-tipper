use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Custom(String),

    #[error(transparent)]
    Bip32(#[from] spectre_bip32::Error),

    #[error(transparent)]
    WalletError(#[from] spectre_wallet_core::error::Error),

    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error("Transition Wallet Already Exists")]
    TransitionWalletAlreadyExists(),

    #[error("Owned Wallet Already Exists")]
    OwnedWalletAlreadyExists(),

    #[error("Owned Wallet Not Found")]
    OwnedWalletNotFound(),
}

impl Error {
    pub fn custom<T: Into<String>>(msg: T) -> Self {
        Error::Custom(msg.into())
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Self::Custom(err)
    }
}

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Self::Custom(err.to_string())
    }
}
