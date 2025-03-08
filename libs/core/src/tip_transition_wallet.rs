use std::{sync::Arc, time::SystemTime};

use crate::tip_context::TipContext;
use crate::{result::Result, transition_wallet_metadata::TransitionWalletMetadata};
use spectre_addresses::Address;
use spectre_wallet_core::{
    prelude::{EncryptionKind, Language, Mnemonic, WordCount},
    rpc::{Rpc, RpcCtl},
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
    opened_at: SystemTime,
}

impl TipTransitionWallet {
    pub fn new(
        initiator_identifier: String,
        target_identifier: String,
        wallet: Arc<Wallet>,
        receive_address: Address,
    ) -> Self {
        TipTransitionWallet {
            opened_at: SystemTime::now(),
            initiator_identifier,
            target_identifier,
            receive_address,
            wallet,
        }
    }

    pub async fn create(
        tip_context: Arc<TipContext>,
        initiator_identifier: &str,
        target_identifier: &str,
    ) -> Result<TipTransitionWallet> {
        // @TODO(izio): generate randomly
        let secret_str: String = "test_secret".into();

        let wallet_secret = Secret::from(secret_str.clone());
        let wallet_identifier =
            format!("transition-{}-{}", target_identifier, initiator_identifier);

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

        tip_wallet.bind_rpc(&tip_context).await?;

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
            format!("transition-{}-{}", target_identifier, initiator_identifier);

        let wallet = Wallet::try_new(
            localstore,
            Some(tip_context.resolver()),
            Some(tip_context.network_id()),
        )?;
        let wallet_arc = Arc::new(wallet.clone());

        let args = WalletOpenArgs::default_with_legacy_accounts();

        {
            let guard = wallet_arc.guard();
            let guard = guard.lock().await;
            wallet_arc
                .open(wallet_secret, Some(wallet_identifier), args, &guard)
                .await?;
        }

        {
            let guard = wallet_arc.guard();
            let guard = guard.lock().await;
            wallet_arc.activate_accounts(None, &guard).await?;
        }

        wallet_arc.autoselect_default_account_if_single().await?;

        let receive_address = wallet_arc.account()?.receive_address()?;

        let tip_wallet = TipTransitionWallet::new(
            initiator_identifier.into(),
            target_identifier.into(),
            wallet_arc,
            receive_address,
        );

        tip_wallet.bind_rpc(&tip_context).await?;

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

    async fn bind_rpc(&self, tip_context: &Arc<TipContext>) -> Result<&Self> {
        // bind context rpc into wallet
        let ctl = RpcCtl::new();

        let rpc = Rpc::new(tip_context.rpc_api(), ctl);

        self.wallet.bind_rpc(Some(rpc)).await?;

        // initiate utxo processor and load initiate account balance
        self.wallet
            .account()?
            .utxo_context()
            .processor()
            .handle_connect()
            .await?;

        self.wallet.account()?.scan(None, None).await?;

        Ok(self)
    }
}
