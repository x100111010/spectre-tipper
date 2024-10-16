use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use spectre_wallet_core::wallet::Wallet;
use spectre_wrpc_client::{prelude::NetworkId, Resolver};

use crate::result::Result;

pub struct TipContext {
    resolver: Resolver,
    network_id: NetworkId,
    opened_wallet: RwLock<HashMap<String, Wallet>>,
}

impl TipContext {
    pub async fn try_new_arc(resolver: Resolver, network_id: NetworkId) -> Result<Arc<Self>> {
        Ok(Arc::new(TipContext {
            network_id,
            resolver,
            opened_wallet: RwLock::new(HashMap::new()),
        }))
    }

    pub fn network_id(&self) -> NetworkId {
        self.network_id.clone()
    }

    pub fn resolver(&self) -> Resolver {
        self.resolver.clone()
    }

    pub fn get_opened_wallet_rw_lock(&self) -> &RwLock<HashMap<String, Wallet>> {
        return &self.opened_wallet;
    }

    pub fn add_opened_wallet(&self, identifier: String, wallet: Wallet) {
        let mut lock = self.opened_wallet.write().unwrap();
        lock.insert(identifier, wallet);
    }

    pub fn remove_opened_wallet(&self, identifier: String) {
        let mut lock = self.opened_wallet.write().unwrap();
        lock.remove(&identifier);
    }
}
