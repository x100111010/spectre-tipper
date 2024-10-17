use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Bip32(#[from] spectre_bip32::Error),

    #[error(transparent)]
    WalletError(#[from] spectre_wallet_core::error::Error),
}
