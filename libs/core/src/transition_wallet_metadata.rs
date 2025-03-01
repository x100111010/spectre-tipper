use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use spectre_addresses::Address;
use tokio::sync::RwLock;

use crate::{error::Error, result::Result};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransitionWalletMetadata {
    pub identifier: String,
    pub target_identifier: String,
    pub initiator_identifier: String,
    pub receive_address: Address,
    // @TODO(izio): maybe hide this
    pub secret: String,
}

impl TransitionWalletMetadata {
    pub fn new(
        identifier: String,
        target_identifier: String,
        initiator_identifier: String,
        receive_address: Address,
        secret: String,
    ) -> Self {
        TransitionWalletMetadata {
            identifier,
            initiator_identifier,
            receive_address,
            secret,
            target_identifier,
        }
    }
}

#[derive(Debug)]
pub struct TransitionWalletMetadataStore {
    metadata: RwLock<Vec<TransitionWalletMetadata>>,
    path_buf: PathBuf,
}

impl TransitionWalletMetadataStore {
    pub fn new(path_buf: &PathBuf) -> Result<Self> {
        let path = Path::new(path_buf);

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => {
                let mut created_file = File::create(path)?;

                created_file.write(b"[]")?;

                File::open(path)?
            }
        };

        let metadata: Vec<TransitionWalletMetadata> = serde_json::from_reader(file)?;

        Ok(TransitionWalletMetadataStore {
            metadata: RwLock::new(metadata),
            path_buf: path_buf.clone(),
        })
    }

    pub async fn add(&self, transition_wallet_metadata: &TransitionWalletMetadata) -> Result<()> {
        let mut metadata = self.metadata.write().await;

        if metadata
            .iter()
            .any(|metadata| metadata.identifier == transition_wallet_metadata.identifier)
        {
            return Err(Error::TransitionWalletAlreadyExists());
        }

        let file = File::create(Path::new(&self.path_buf))?;

        metadata.push(transition_wallet_metadata.clone());

        let copied = metadata.clone();

        serde_json::to_writer(file, &copied)?;

        Ok(())
    }

    pub async fn find_transition_wallet_metadata_by_recipiant(
        &self,
        recipiant: Address,
    ) -> Result<Vec<TransitionWalletMetadata>> {
        let all_metadata = self.metadata.read().await;
        let metadata: Vec<TransitionWalletMetadata> = all_metadata
            .iter()
            .filter(|metadata| metadata.receive_address == recipiant)
            .cloned()
            .collect();

        Ok(metadata)
    }

    pub async fn find_transition_wallet_metadata_by_target_identifier(
        &self,
        target_identifier: &str,
    ) -> Result<Vec<TransitionWalletMetadata>> {
        let all_metadata = self.metadata.read().await;
        let metadata: Vec<TransitionWalletMetadata> = all_metadata
            .iter()
            .filter(|metadata| metadata.target_identifier == target_identifier)
            .cloned()
            .collect();

        Ok(metadata)
    }

    pub async fn find_transition_wallet_metadata_by_identifier_couple(
        &self,
        initiator_identifier: &str,
        target_identifier: &str,
    ) -> Result<Option<TransitionWalletMetadata>> {
        let all_metadata = self.metadata.read().await;
        let metadata: Option<TransitionWalletMetadata> = all_metadata
            .iter()
            .find(|&metadata| {
                metadata.initiator_identifier == initiator_identifier
                    && metadata.target_identifier == target_identifier
            })
            .cloned();
        Ok(metadata)
    }
}
