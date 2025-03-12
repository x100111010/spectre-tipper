use std::{fmt::Display, sync::Arc};

use futures_util::TryStreamExt;
use spectre_bip32::secp256k1::rand::{
    self,
    distributions::{Alphanumeric, DistString},
};
use spectre_consensus_core::constants::SOMPI_PER_SPECTRE;
use spectre_wallet_core::{
    prelude::Account,
    rpc::{Rpc, RpcApi, RpcCtl},
    tx::{Fees, Generator, GeneratorSettings, GeneratorSummary, PaymentOutputs},
    wallet::Wallet,
};
use spectre_wrpc_client::prelude::NetworkType;
use tokio::task::yield_now;

use crate::{error::Error, result::Result};

pub fn try_parse_required_nonzero_spectre_as_sompi_u64<S: ToString + Display>(
    spectre_amount: Option<S>,
) -> Result<u64> {
    if let Some(spectre_amount) = spectre_amount {
        let sompi_amount = spectre_amount.to_string().parse::<f64>().map_err(|_| {
            Error::custom(format!(
                "Supplied Spectre amount is not valid: '{spectre_amount}'"
            ))
        })? * SOMPI_PER_SPECTRE as f64;
        if sompi_amount < 0.0 {
            Err(Error::custom(
                "Supplied Spectre amount is not valid: '{spectre_amount}'",
            ))
        } else {
            let sompi_amount = sompi_amount as u64;
            if sompi_amount == 0 {
                Err(Error::custom(
                    "Supplied required Spectre amount must not be a zero: '{spectre_amount}'",
                ))
            } else {
                Ok(sompi_amount)
            }
        }
    } else {
        Err(Error::custom("Missing Spectre amount"))
    }
}

pub fn generate_random_transition_wallet_secret() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 12)
}

pub async fn connect_wallet_to_rpc(wallet: &Arc<Wallet>, rpc_api: Arc<dyn RpcApi>) -> Result<()> {
    let ctl = RpcCtl::new();
    ctl.signal_open().await?;

    let rpc = Rpc::new(rpc_api, ctl);

    wallet.bind_rpc(Some(rpc)).await?;

    Ok(())
}

pub fn build_transition_wallet_identifier(
    target_identifier: &str,
    initiator_identifier: &str,
) -> String {
    format!("transition-{}-{}", target_identifier, initiator_identifier)
}

pub async fn estimate_fees(
    account: &Arc<dyn Account>,
    payment_outputs: PaymentOutputs,
) -> Result<GeneratorSummary> {
    let settings = GeneratorSettings::try_new_with_account(
        account.clone(),
        payment_outputs.into(),
        Fees::ReceiverPays(0),
        None,
    )?;

    let generator = Generator::try_new(settings, None, None)?;

    let mut stream = generator.stream();
    while let Some(_transaction) = stream.try_next().await? {
        println!("ok: {}", _transaction.id());
        yield_now().await;
    }

    Ok(generator.summary())
}

pub fn get_tx_explorer_url(tx_id: &str, network_type: NetworkType) -> String {
    let sub_domain = match network_type {
        NetworkType::Mainnet => "explorer",
        _ => "explorer-tn",
    };

    format!("https://{}.spectre-network.org/txs/{}", sub_domain, tx_id)
}
