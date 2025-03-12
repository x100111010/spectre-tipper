use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use spectre_wallet_core::{rpc::RpcApi, storage::Interface, wallet::Wallet};
use spectre_wrpc_client::{prelude::NetworkId, Resolver, SpectreRpcClient};
use tracing::info;

use crate::{
    owned_wallet_metadata::OwnedWalletMetadataStore, result::Result,
    tip_owned_wallet::TipOwnedWallet, transition_wallet_metadata::TransitionWalletMetadataStore,
};

pub struct TipContext {
    resolver: Resolver,
    network_id: NetworkId,
    opened_owned_wallets: RwLock<HashMap<String, TipOwnedWallet>>,
    pub transition_wallet_metadata_store: TransitionWalletMetadataStore,
    pub owned_wallet_metadata_store: OwnedWalletMetadataStore,
    forced_node_url: Option<String>,
    wrpc_client: Arc<SpectreRpcClient>,
}

impl TipContext {
    pub fn try_new_arc(
        resolver: Resolver,
        network_id: NetworkId,
        forced_node_url: Option<String>,
        wrpc_client: Arc<SpectreRpcClient>,
        wallet_data_path_buf: PathBuf,
    ) -> Result<Arc<Self>> {
        let transition_wallet_metadata_path_buf =
            wallet_data_path_buf.clone().join("transitions.json");
        let owned_wallet_metadata_path_buf = wallet_data_path_buf.clone().join("owned.json");

        info!(
            "Using {} as owned wallet metadata store",
            owned_wallet_metadata_path_buf.to_str().unwrap()
        );

        info!(
            "Using {} as transition wallet metadata store",
            transition_wallet_metadata_path_buf.to_str().unwrap()
        );

        let transition_wallet_metadata_store =
            TransitionWalletMetadataStore::new(&transition_wallet_metadata_path_buf)?;

        let owned_wallet_metadata_store =
            OwnedWalletMetadataStore::new(&owned_wallet_metadata_path_buf)?;

        Ok(Arc::new(TipContext {
            network_id,
            resolver,
            forced_node_url,
            wrpc_client,
            opened_owned_wallets: RwLock::new(HashMap::new()),
            transition_wallet_metadata_store,
            owned_wallet_metadata_store,
        }))
    }

    pub fn network_id(&self) -> NetworkId {
        self.network_id
    }

    pub fn resolver(&self) -> Resolver {
        self.resolver.clone()
    }

    pub fn get_opened_owned_wallet_rw_lock(&self) -> &RwLock<HashMap<String, TipOwnedWallet>> {
        &self.opened_owned_wallets
    }

    pub fn does_opened_owned_wallet_exists(&self, identifier: &str) -> bool {
        let read_lock = self.opened_owned_wallets.read().unwrap();
        read_lock.contains_key(identifier)
    }

    /**
     * return a cloned version of the wallet, if found
     */
    pub fn get_opened_owned_wallet(&self, identifier: &str) -> Option<TipOwnedWallet> {
        let read_lock = self.opened_owned_wallets.read().unwrap();
        read_lock.get(identifier).cloned()
    }

    pub fn add_opened_owned_wallet(
        &self,
        identifier: String,
        wallet: TipOwnedWallet,
    ) -> TipOwnedWallet {
        let mut lock = self.opened_owned_wallets.write().unwrap();
        lock.insert(identifier, wallet.clone());
        wallet
    }

    /*
     * closing the wallet has to be done externally
     */
    pub fn remove_opened_owned_wallet(&self, identifier: &str) -> Option<TipOwnedWallet> {
        let mut lock = self.opened_owned_wallets.write().unwrap();
        lock.remove(identifier)
    }

    /*
     * get a new store
     */
    pub fn local_store(&self) -> Result<Arc<dyn Interface>> {
        Ok(Wallet::local_store()?)
    }

    pub fn forced_node_url(&self) -> Option<String> {
        self.forced_node_url.clone()
    }

    pub fn rpc_api(&self) -> Arc<dyn RpcApi> {
        self.wrpc_client.clone()
    }
}
