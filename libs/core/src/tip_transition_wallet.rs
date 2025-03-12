use std::sync::Arc;

use crate::tip_context::TipContext;
use crate::utils::{
    build_transition_wallet_identifier, connect_wallet_to_rpc,
    generate_random_transition_wallet_secret,
};
use crate::{result::Result, transition_wallet_metadata::TransitionWalletMetadata};
use spectre_addresses::Address;
use spectre_wallet_core::{
    prelude::{EncryptionKind, Language, Mnemonic, WordCount},
    storage::PrvKeyData,
    wallet::{AccountCreateArgsBip32, Wallet, WalletCreateArgs, WalletOpenArgs},
};
use spectre_wallet_keys::secret::Secret;

#[derive(Clone)]
pub struct TipTransitionWallet {
    target_identifier: String,
    initiator_identifier: String,
    wallet: Arc<Wallet>,
    receive_address: Address,
}

impl TipTransitionWallet {
    pub fn new(
        initiator_identifier: String,
        target_identifier: String,
        wallet: Arc<Wallet>,
        receive_address: Address,
    ) -> Self {
        TipTransitionWallet {
            initiator_identifier,
            target_identifier,
            receive_address,
            wallet,
        }
    }

    /**
     * Note: created transition wallet aren't connected to RPC by default, as there is no use for this
     */
    pub async fn create(
        tip_context: Arc<TipContext>,
        initiator_identifier: &str,
        target_identifier: &str,
    ) -> Result<TipTransitionWallet> {
        let secret_str: String = generate_random_transition_wallet_secret();

        let wallet_secret = Secret::from(secret_str.clone());
        let wallet_identifier =
            build_transition_wallet_identifier(target_identifier, initiator_identifier);

        let mnemonic = Mnemonic::random(WordCount::Words12, Language::default())?;
        let localstore = Wallet::local_store()?;

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;

        let wallet_arc = Arc::new(wallet.clone());

        let wallet_args: WalletCreateArgs = WalletCreateArgs::new(
            Some(wallet_identifier.clone()),
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
            .store(&wallet_secret, prv_key_data)
            .await?;
        wallet_arc.store().commit(&wallet_secret).await?;

        let account_args = AccountCreateArgsBip32::new(None, None);
        let account = wallet_arc
            .create_account_bip32(&wallet_secret, prv_key_data_id, None, account_args)
            .await?;

        let receive_address = account.receive_address()?;

        wallet_arc.store().flush(&wallet_secret).await?;

        let guard = wallet_arc.guard();
        let guard = guard.lock().await;
        wallet_arc.activate_accounts(None, &guard).await?;

        wallet_arc.autoselect_default_account_if_single().await?;

        let tip_wallet = TipTransitionWallet::new(
            initiator_identifier.into(),
            target_identifier.into(),
            wallet_arc,
            receive_address,
        );

        tip_context
            .transition_wallet_metadata_store
            .add(&TransitionWalletMetadata::new(
                wallet_identifier,
                target_identifier.into(),
                initiator_identifier.into(),
                tip_wallet.receive_address(),
                secret_str,
            ))
            .await?;

        Ok(tip_wallet)
    }

    pub async fn open(
        tip_context: Arc<TipContext>,
        wallet_secret: &Secret,
        initiator_identifier: &str,
        target_identifier: &str,
    ) -> Result<TipTransitionWallet> {
        let localstore = Wallet::local_store()?;

        let wallet_identifier =
            build_transition_wallet_identifier(target_identifier, initiator_identifier);

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;
        let wallet_arc = Arc::new(wallet.clone());

        connect_wallet_to_rpc(&wallet_arc, tip_context.rpc_api()).await?;

        let args = WalletOpenArgs::default_with_legacy_accounts();

        {
            let guard = wallet_arc.guard();
            let guard = guard.lock().await;

            wallet_arc
                .open(wallet_secret, Some(wallet_identifier), args, &guard)
                .await?;

            wallet_arc.start().await?;

            wallet_arc.activate_accounts(None, &guard).await?;
            wallet_arc.autoselect_default_account_if_single().await?;
        }

        let receive_address = wallet_arc.account()?.receive_address()?;

        wallet_arc
            .account()?
            .utxo_context()
            .register_addresses(&[receive_address.clone()])
            .await?;

        let tip_wallet = TipTransitionWallet::new(
            initiator_identifier.into(),
            target_identifier.into(),
            wallet_arc,
            receive_address,
        );

        Ok(tip_wallet)
    }

    pub fn target_identifier(&self) -> &str {
        &self.target_identifier
    }

    pub fn wallet(&self) -> Arc<Wallet> {
        self.wallet.clone()
    }

    pub fn receive_address(&self) -> Address {
        self.receive_address.clone()
    }

    pub fn wallet_identifier(&self) -> String {
        build_transition_wallet_identifier(&self.target_identifier, &self.initiator_identifier)
    }
}
