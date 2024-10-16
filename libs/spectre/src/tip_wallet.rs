use std::sync::Arc;

use crate::result::Result;
use models::tip_context::TipContext;
use spectre_addresses::Address;
use spectre_wallet_core::{
    prelude::{EncryptionKind, Language, Mnemonic, WordCount},
    storage::PrvKeyData,
    wallet::{AccountCreateArgsBip32, Wallet, WalletCreateArgs, WalletOpenArgs},
};
use spectre_wallet_keys::secret::Secret;

pub struct TipOwnedWallet {
    owned_identifier: String,
    wallet: Arc<Wallet>,
    receive_address: Address,
}

impl TipOwnedWallet {
    pub async fn create(
        tip_context: Arc<TipContext>,
        wallet_secret: &Secret,
        owned_identifier: &str,
    ) -> Result<(TipOwnedWallet, Secret)> {
        let mnemonic = Mnemonic::random(WordCount::Words12, Language::default())?;
        let mnemonic_secret = Secret::from(mnemonic.clone().phrase());

        let localstore = Wallet::local_store()?;

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;

        let wallet_arc = Arc::new(wallet.clone());

        let wallet_args: WalletCreateArgs = WalletCreateArgs::new(
            Some(owned_identifier.into()),
            None,
            EncryptionKind::XChaCha20Poly1305,
            None,
            true,
        );

        wallet_arc.store().batch().await?;

        wallet_arc
            .create_wallet(&wallet_secret, wallet_args)
            .await?;

        let prv_key_data = PrvKeyData::try_from_mnemonic(
            mnemonic.clone(),
            None,
            // unused since payment_secret is None
            EncryptionKind::XChaCha20Poly1305,
        )?;
        let prv_key_data_id = prv_key_data.id;

        let prv_key_data_store = wallet_arc.store().as_prv_key_data_store()?;
        prv_key_data_store
            .store(wallet_secret, prv_key_data)
            .await?;
        wallet_arc.store().commit(wallet_secret).await?;

        let account_args = AccountCreateArgsBip32::new(None, None);
        let account = wallet_arc
            .create_account_bip32(wallet_secret, prv_key_data_id, None, account_args)
            .await?;

        let receive_address = account.receive_address()?;

        wallet_arc.store().flush(&wallet_secret).await?;

        wallet_arc.activate_accounts(None).await?;

        tip_context.add_opened_wallet(owned_identifier.into(), wallet);

        return Ok((
            TipOwnedWallet {
                owned_identifier: owned_identifier.into(),
                wallet: wallet_arc,
                receive_address,
            },
            mnemonic_secret,
        ));
    }

    pub async fn open(
        tip_context: Arc<TipContext>,
        wallet_secret: &Secret,
        owned_identifier: &str,
    ) -> Result<TipOwnedWallet> {
        let localstore = Wallet::local_store()?;

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;
        let wallet_arc = Arc::new(wallet.clone());

        let args = WalletOpenArgs::default_with_legacy_accounts();
        wallet_arc
            .open(&wallet_secret, Some(owned_identifier.into()), args)
            .await?;
        wallet_arc.activate_accounts(None).await?;

        wallet_arc.autoselect_default_account_if_single().await?;

        let receive_address = wallet_arc.account()?.receive_address()?;

        tip_context.add_opened_wallet(owned_identifier.into(), wallet);

        return Ok(TipOwnedWallet {
            owned_identifier: owned_identifier.into(),
            receive_address: receive_address,
            wallet: wallet_arc,
        });
    }

    /**
     * restore a wallet from a mnemonic
     * override any already existing wallet owned by `owned_identifier`
     */
    pub async fn restore(
        tip_context: Arc<TipContext>,
        wallet_secret: &Secret,
        mnemonic: Mnemonic,
        owned_identifier: &str,
    ) -> Result<TipOwnedWallet> {
        let localstore = Wallet::local_store()?;

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;

        let wallet_arc = Arc::new(wallet.clone());

        let wallet_args: WalletCreateArgs = WalletCreateArgs::new(
            Some(owned_identifier.into()),
            None,
            EncryptionKind::XChaCha20Poly1305,
            None,
            true,
        );

        wallet_arc.store().batch().await?;

        wallet_arc
            .create_wallet(&wallet_secret, wallet_args)
            .await?;

        let prv_key_data = PrvKeyData::try_from_mnemonic(
            mnemonic.clone(),
            None,
            // unused since payment_secret is None
            EncryptionKind::XChaCha20Poly1305,
        )?;
        let prv_key_data_id = prv_key_data.id;

        let prv_key_data_store = wallet_arc.store().as_prv_key_data_store()?;
        prv_key_data_store
            .store(wallet_secret, prv_key_data)
            .await?;
        wallet_arc.store().commit(wallet_secret).await?;

        let account_args = AccountCreateArgsBip32::new(None, None);
        let account = wallet_arc
            .create_account_bip32(wallet_secret, prv_key_data_id, None, account_args)
            .await?;

        let receive_address = account.receive_address()?;

        wallet_arc.store().flush(&wallet_secret).await?;

        wallet_arc.activate_accounts(None).await?;

        tip_context.add_opened_wallet(owned_identifier.into(), wallet);

        return Ok(TipOwnedWallet {
            owned_identifier: owned_identifier.into(),
            wallet: wallet_arc,
            receive_address,
        });
    }

    pub fn owned_identifier(&self) -> &str {
        &self.owned_identifier
    }

    pub fn wallet(&self) -> Arc<Wallet> {
        self.wallet.clone()
    }

    pub fn receive_address(&self) -> Address {
        self.receive_address.clone()
    }
}

pub struct TipTransitionWallet {
    text: String,
}

impl TipTransitionWallet {
    pub fn create() -> TipTransitionWallet {
        return TipTransitionWallet { text: "ok".into() };
    }
}

#[cfg(test)]
mod tests {
    use spectre_wrpc_client::{
        prelude::{NetworkId, NetworkType},
        Resolver,
    };

    use super::*;

    async fn get_ctx() -> Arc<TipContext> {
        TipContext::try_new_arc(Resolver::default(), NetworkId::new(NetworkType::Mainnet))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_create_wallet() {
        TipOwnedWallet::create(get_ctx().await, &Secret::from("value"), "identifier")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_open_wallet() {
        TipOwnedWallet::create(get_ctx().await, &Secret::from("value"), "identifier2")
            .await
            .unwrap();
        TipOwnedWallet::open(get_ctx().await, &Secret::from("value"), "identifier2")
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_open_wallet_with_wrong_secret() {
        let _ =
            TipOwnedWallet::create(get_ctx().await, &Secret::from("value"), "identifier3").await;
        TipOwnedWallet::open(get_ctx().await, &Secret::from("value2"), "identifier3")
            .await
            .unwrap();
    }

    #[tokio::test]
    #[should_panic]
    async fn test_open_wallet_with_innexistant_wallet() {
        TipOwnedWallet::open(
            get_ctx().await,
            &Secret::from("value2"),
            "identifier_innexistant",
        )
        .await
        .unwrap();
    }
}
