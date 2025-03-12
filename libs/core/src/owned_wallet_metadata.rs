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
pub struct OwnedWalletMetadata {
    pub owner_identifier: String,
    pub receive_address: Address,
}

impl OwnedWalletMetadata {
    pub fn new(owner_identifier: String, receive_address: Address) -> Self {
        OwnedWalletMetadata {
            owner_identifier,
            receive_address,
        }
    }
}

#[derive(Debug)]
pub struct OwnedWalletMetadataStore {
    metadata: RwLock<Vec<OwnedWalletMetadata>>,
    path_buf: PathBuf,
}

impl OwnedWalletMetadataStore {
    pub fn new(path_buf: &PathBuf) -> Result<Self> {
        let path = Path::new(path_buf);

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => {
                let mut created_file = File::create(path)?;

                created_file.write_all(b"[]")?;

                File::open(path)?
            }
        };

        let metadata: Vec<OwnedWalletMetadata> = serde_json::from_reader(file)?;

        Ok(OwnedWalletMetadataStore {
            metadata: RwLock::new(metadata),
            path_buf: path_buf.clone(),
        })
    }

    pub async fn add(&self, owned_wallet_metadata: &OwnedWalletMetadata) -> Result<()> {
        let mut metadata = self.metadata.write().await;

        if metadata
            .iter()
            .any(|metadata| metadata.owner_identifier == owned_wallet_metadata.owner_identifier)
        {
            return Err(Error::OwnedWalletAlreadyExists());
        }

        let file = File::create(Path::new(&self.path_buf))?;

        metadata.push(owned_wallet_metadata.clone());

        let copied = metadata.clone();

        serde_json::to_writer(file, &copied)?;

        Ok(())
    }

    pub async fn remove_by_owner_identifier(&self, owner_identifier: String) -> Result<()> {
        let mut metadata = self.metadata.write().await;

        let metadata_to_delete = metadata
            .iter()
            .position(|metadata| metadata.owner_identifier == owner_identifier);

        if metadata_to_delete.is_none() {
            return Ok(());
        }

        metadata.swap_remove(metadata_to_delete.unwrap());

        let file = File::create(Path::new(&self.path_buf))?;

        let copied = metadata.clone();

        serde_json::to_writer(file, &copied)?;

        Ok(())
    }

    pub async fn find_owned_wallet_metadata_by_recipient_address(
        &self,
        recipient: Address,
    ) -> Result<OwnedWalletMetadata> {
        let all_metadata = self.metadata.read().await;
        let metadata_option: Option<OwnedWalletMetadata> = all_metadata
            .iter()
            .find(|&metadata| metadata.receive_address == recipient)
            .cloned();

        if metadata_option.is_none() {
            return Err(Error::OwnedWalletNotFound());
        }

        Ok(metadata_option.unwrap())
    }

    pub async fn find_owned_wallet_metadata_by_owner_identifier(
        &self,
        owner_identifier: &str,
    ) -> Result<OwnedWalletMetadata> {
        let all_metadata = self.metadata.read().await;
        let metadata_option: Option<OwnedWalletMetadata> = all_metadata
            .iter()
            .find(|metadata| metadata.owner_identifier == owner_identifier)
            .cloned();

        if metadata_option.is_none() {
            return Err(Error::OwnedWalletNotFound());
        }

        Ok(metadata_option.unwrap())
    }
}
