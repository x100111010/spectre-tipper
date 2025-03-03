use std::{env, str::FromStr, sync::Arc};

use core::{
    error::Error as SpectreError, tip_context::TipContext, tip_owned_wallet::TipOwnedWallet,
    tip_transition_wallet::TipTransitionWallet,
    utils::try_parse_required_nonzero_spectre_as_sompi_u64,
};
use directories::BaseDirs;
use futures::future::join_all;
use poise::{
    serenity_prelude::{self as serenity, Colour, CreateEmbed},
    CreateReply, Modal,
};
use tokio::fs;

use spectre_wallet_core::{
    prelude::{Language, Mnemonic},
    rpc::ConnectOptions,
    settings::application_folder,
    tx::{Fees, PaymentOutputs},
    wallet::Wallet,
};
use spectre_wallet_keys::secret::Secret;
use spectre_wrpc_client::{prelude::NetworkId, Resolver, SpectreRpcClient, WrpcEncoding};
use workflow_core::abortable::Abortable;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

// TODO: mutualize embed creation (avoid repetition and centralize calls) and reply in general
// TODO: move cmd to dedicated files

#[poise::command(
    slash_command,
    subcommands(
        "create", "open", "close", "restore", "status", "destroy", "send", "debug"
    ),
    category = "wallet"
)]
/// Main command for interracting with the discord wallet
async fn wallet(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// create (initiate) a fresh discord wallet with a secret
async fn create(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "secret"]
    secret: String,
) -> Result<(), Error> {
    let embed = CreateEmbed::new();

    if secret.len() < 10 {
        let errored_embed = embed
            .clone()
            .title("Error while restoring the wallet")
            .description("Secret must be greater than 10")
            .colour(Colour::DARK_RED);

        ctx.send(CreateReply {
            reply: false,
            embeds: vec![errored_embed],
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if is_initiated {
        ctx.send(CreateReply {
            reply: false,
            content: Some("A discord wallet already exists".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let (tip_wallet, mnemonic) = TipOwnedWallet::create(
        tip_context.clone(),
        &Secret::from(secret),
        &wallet_owner_identifier,
    )
    .await?;

    let response_message = format!("{}\n{}", mnemonic.phrase(), tip_wallet.receive_address());

    ctx.send(CreateReply {
        reply: false,
        content: Some(response_message),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// open the discord wallet using the secret
/// TODO: display balance?
async fn open(
    ctx: Context<'_>,
    #[min_length = 10]
    #[description = "secret"]
    secret: String,
) -> Result<(), Error> {
    let embed = CreateEmbed::new();

    if secret.len() < 10 {
        let errored_embed = embed
            .clone()
            .title("Error while restoring the wallet")
            .description("Secret must be greater than 10")
            .colour(Colour::DARK_RED);

        ctx.send(CreateReply {
            reply: false,
            embeds: vec![errored_embed],
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    // already opened
    if let Some(wallet) = tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
        ctx.send(CreateReply {
            reply: false,
            content: Some(format!("{}", wallet.receive_address())),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let tip_wallet_result = TipOwnedWallet::open(
        tip_context.clone(),
        &Secret::from(secret),
        &wallet_owner_identifier,
    )
    .await;

    let tip_wallet = match tip_wallet_result {
        Ok(t) => t,
        Err(SpectreError::WalletError(spectre_wallet_core::error::Error::WalletDecrypt(_))) => {
            let errored_embed = embed
                .clone()
                .title("Error while opening the wallet")
                .description("Secret is wrong")
                .colour(Colour::DARK_RED);

            ctx.send(CreateReply {
                reply: false,
                embeds: vec![errored_embed],
                ephemeral: Some(true),
                ..Default::default()
            })
            .await?;

            return Ok(());
        }
        Err(error) => return Err(Error::from(error)),
    };

    // should this be ephemeral? leaks secret
    ctx.send(CreateReply {
        reply: false,
        content: Some(format!("{}", tip_wallet.receive_address())),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// close the opened discord wallet
async fn close(ctx: Context<'_>) -> Result<(), Error> {
    let tip_context = ctx.data;

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);

    if is_opened {
        let tip_wallet_result = tip_context.remove_opened_owned_wallet(&wallet_owner_identifier);

        if let Some(tip_wallet) = tip_wallet_result {
            tip_wallet.wallet().close().await?;
        }
    }

    ctx.send(CreateReply {
        reply: false,
        content: Some("wallet closed".to_string()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// get the status of your discord wallet
async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if !is_initiated {
        ctx.send(CreateReply {
            reply: false,
            content: Some(
                "The wallet has not been created yet. Use the `create` command to create a wallet."
                    .to_string(),
            ),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    if !is_opened {
        ctx.send(CreateReply {
            reply: false,
            content: Some("The wallet is not opened. Use the `open` command to open the wallet and display its balance.".to_string()),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let balance: u64 = {
        let mut b: u64 = 0;
        if let Some(owned_wallet) = tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
            if let Ok(account) = owned_wallet.wallet().account() {
                if let Some(balance) = account.balance() {
                    b = balance.mature;
                }
            }
        }
        b
    };

    let transition_wallets = tip_context
        .transition_wallet_metadata_store
        .find_transition_wallet_metadata_by_target_identifier(&wallet_owner_identifier)
        .await?;

    let pending_transition_balance = join_all(transition_wallets.iter().map(|metadata| async {
        let secret = Secret::from(metadata.secret.clone());
        let transition_wallet = TipTransitionWallet::open(
            tip_context.clone(),
            &secret,
            &metadata.initiator_identifier,
            &metadata.target_identifier,
        )
        .await;

        let balance: u64 = match transition_wallet {
            Ok(tw) => {
                let account_result = tw.wallet().account();
                let mut b = 0;
                if let Ok(account) = account_result {
                    if let Some(balance) = account.balance() {
                        b = balance.mature
                    }
                }

                b
            }
            Err(e) => {
                println!("warning: {:?}", e);

                0 as u64
            }
        };

        return balance;
    }))
    .await
    .into_iter()
    .reduce(|a, b| a + b);

    // wallet needs to be opened in order to display its balance
    // else it display 0
    ctx.say(format!(
        "is opened: {}\nis_initiated: {}\nbalance: {}\npending transition balance: {:?}",
        is_opened, is_initiated, balance, pending_transition_balance
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// dev cmd
async fn debug(ctx: Context<'_>) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if !is_opened && !is_initiated {
        ctx.say(format!(
            "is opened: {}\nis_initiated{}",
            is_opened, is_initiated
        ))
        .await?;

        return Ok(());
    }

    let tip_wallet = match tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
        Some(w) => w,
        None => {
            ctx.say("unexpected error: wallet not opened").await?;
            return Ok(());
        }
    };

    let wallet = tip_wallet.wallet();
    let tipee_receive_address = wallet.account().unwrap().receive_address().unwrap();

    ctx.say(format!(
        "tipee receive address: {}",
        tipee_receive_address.address_to_string()
    ))
    .await?;

    // let wallet = Arc::new(Wallet::try_new(Wallet::local_store()?, None, None)?);

    // let descriptors = wallet.account_descriptors().await?;

    let transition_wallets = tip_context
        .transition_wallet_metadata_store
        .find_transition_wallet_metadata_by_target_identifier(&wallet_owner_identifier)
        .await?;

    join_all(transition_wallets.iter().map(|metadata| async {
        let secret = Secret::from(metadata.secret.clone());
        let transition_wallet = TipTransitionWallet::open(
            tip_context.clone(),
            &secret,
            &metadata.initiator_identifier,
            &metadata.target_identifier,
        )
        .await;

        match transition_wallet {
            Ok(tw) => {
                let account_result = tw.wallet().account();
                if let Ok(account) = account_result {
                    if let Some(balance) = account.balance() {
                        let receive_address = tipee_receive_address.clone();

                        println!(
                            "sending {} SPR from {} to {}",
                            balance.mature,
                            account.receive_address().unwrap().address_to_string(),
                            tipee_receive_address.address_to_string()
                        );

                        let address = receive_address;

                        let amount_sompi = balance.mature;
                        // 10_000 is arbitrary and could/should be estimated before hand
                        // https://kaspa-mdbook.aspectron.com/transactions/constraints/fees.html
                        let amount_minus_gas_fee = amount_sompi - 10000;

                        println!(
                            "amount sompi {}, with gas fee {}",
                            amount_sompi, amount_minus_gas_fee
                        );

                        let outputs = PaymentOutputs::from((address, amount_minus_gas_fee));

                        let abortable = Abortable::default();

                        let wallet_secret = Secret::from(secret);

                        let (summary, hashes) = account
                            .send(
                                outputs.into(),
                                Fees::SenderPays(0),
                                None,
                                wallet_secret,
                                None,
                                &abortable,
                                Some(Arc::new(
                                    move |ptx: &spectre_wallet_core::tx::PendingTransaction| {
                                        println!("tx notifier: {:?}", ptx);
                                    },
                                )),
                            )
                            .await
                            .unwrap();

                        println!("summary {:?}, hashes: {:?}", summary, hashes);
                    }
                }
            }
            Err(e) => {
                println!("warning: {:?}", e);
            }
        };
    }))
    .await;

    ctx.say(format!(
        "is opened: {}\nis_initiated{}",
        is_opened, is_initiated
    ))
    .await?;

    Ok(())
}

#[derive(Debug, poise::Modal)]
#[name = "Confirm wallet destruction"]
struct DestructionModalConfirmation {
    #[name = "write destroy to confirm"]
    first_input: String,
}

#[poise::command(slash_command, category = "wallet")]
/// destroy your existing (if exists) discord wallet
async fn destroy(ctx: Context<'_>) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if !is_initiated {
        ctx.send(CreateReply {
            reply: false,
            content: Some(
                "The wallet is not initiated, cannot destroy a non-existing thing.".to_string(),
            ),
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;

        return Ok(());
    }

    let result = DestructionModalConfirmation::execute(ctx).await?;

    if let Some(data) = result {
        if data.first_input == "destroy" {
            if is_opened {
                let tip_wallet_result =
                    tip_context.remove_opened_owned_wallet(&wallet_owner_identifier);

                if let Some(tip_wallet) = tip_wallet_result {
                    tip_wallet.wallet().close().await?;
                };
            }

            // remove from store
            tip_context
                .owned_wallet_metadata_store
                .remove_by_owner_identifier(wallet_owner_identifier.clone())
                .await?;

            // delete wallet file
            let wallet_folder = application_folder()?;
            let wallet_file = wallet_folder.join(format!("{}.wallet", wallet_owner_identifier));

            if wallet_file.exists() {
                fs::remove_file(&wallet_file).await?;
            }

            ctx.send(CreateReply {
                reply: false,
                content: Some("Wallet destroyed successfully.".to_string()),
                ephemeral: Some(true),
                ..Default::default()
            })
            .await?;

            return Ok(());
        }
    }

    ctx.send(CreateReply {
        reply: false,
        content: Some("Wallet destruction aborted.".to_string()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
/// restore a wallet from the mnemonic
async fn restore(
    ctx: Context<'_>,
    #[description = "mnemonic"] mnemonic_phrase: String,
    #[min_length = 10]
    #[description = "new secret"]
    secret: String,
) -> Result<(), Error> {
    let embed = CreateEmbed::new();

    if secret.len() < 10 {
        let errored_embed = embed
            .clone()
            .title("Error while restoring the wallet")
            .description("Secret must be greater than 10")
            .colour(Colour::DARK_RED);

        ctx.send(CreateReply {
            reply: false,
            embeds: vec![errored_embed],
            ephemeral: Some(true),
            ..Default::default()
        })
        .await?;
    }

    let errored_embed = embed
        .clone()
        .title("Error while restoring the wallet")
        .description("Mnemonic is not valid")
        .colour(Colour::DARK_RED);

    let reply = CreateReply {
        reply: false,
        embeds: vec![errored_embed],
        ephemeral: Some(true),
        ..Default::default()
    };

    // try cast mnemonic_prase as Mnemonic
    let mnemonic = match Mnemonic::new(mnemonic_phrase, Language::default()) {
        Ok(r) => r,
        Err(_) => {
            ctx.send(reply).await?;
            return Ok(());
        }
    };

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let recovered_tip_wallet_result = TipOwnedWallet::restore(
        tip_context.clone(),
        &Secret::from(secret),
        mnemonic,
        &wallet_owner_identifier,
    )
    .await?;

    ctx.send(CreateReply {
        reply: false,
        content: Some(recovered_tip_wallet_result.receive_address().to_string()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// send to user the given amount
async fn send(
    ctx: Context<'_>,
    #[description = "Send to"] user: serenity::User,
    #[description = "Amount"] amount: String,
    #[description = "Wallet Secret"] secret: String,
) -> Result<(), Error> {
    if user.bot || user.system {
        ctx.say("user is a bot or a system user").await?;
        return Ok(());
    }

    let recipiant_identifier = user.id.to_string();

    let response = format!(
        "{}'s account was created at {}",
        user.name,
        user.created_at()
    );
    ctx.say(response).await?;

    let author = ctx.author().id;
    let wallet_owner_identifier = author.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_opened_owned_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

    if !is_initiated {
        ctx.say("wallet not initiated yet").await?;
        return Ok(());
    }

    if !is_opened {
        ctx.say("wallet not opened").await?;
        return Ok(());
    }

    let tip_wallet = match tip_context.get_opened_owned_wallet(&wallet_owner_identifier) {
        Some(w) => w,
        None => {
            ctx.say("unexpected error: wallet not opened").await?;
            return Ok(());
        }
    };

    let wallet = tip_wallet.wallet();

    // find address of recipiant or create a temporary wallet
    let existing_owned_wallet = tip_context
        .owned_wallet_metadata_store
        .find_owned_wallet_metadata_by_owner_identifier(&recipiant_identifier)
        .await;

    let recipiant_address = match existing_owned_wallet {
        Ok(wallet) => wallet.receive_address,
        Err(SpectreError::OwnedWalletNotFound()) => {
            // find or create a temporary wallet
            let transition_wallet_result = tip_context
                .transition_wallet_metadata_store
                .find_transition_wallet_metadata_by_identifier_couple(
                    &author.to_string(),
                    &recipiant_identifier,
                )
                .await?;

            match transition_wallet_result {
                Some(wallet) => wallet.receive_address,
                None => TipTransitionWallet::create(
                    tip_context.clone(),
                    &author.to_string(),
                    &recipiant_identifier,
                )
                .await?
                .receive_address(),
            }
        }
        Err(e) => {
            ctx.say(format!("Error: {:}", e)).await?;
            return Ok(());
        }
    };

    let address = recipiant_address;

    let amount_sompi = try_parse_required_nonzero_spectre_as_sompi_u64(Some(amount))?;

    println!("amount sompi {}", amount_sompi);

    let outputs = PaymentOutputs::from((address, amount_sompi));

    let abortable = Abortable::default();

    let wallet_secret = Secret::from(secret);

    let account = wallet.account()?;

    let (summary, hashes) = account
        .send(
            outputs.into(),
            i64::from(0).into(),
            None,
            wallet_secret,
            None,
            &abortable,
            Some(Arc::new(
                move |ptx: &spectre_wallet_core::tx::PendingTransaction| {
                    println!("tx notifier: {:?}", ptx);
                },
            )),
        )
        .await?;

    ctx.say(format!("{summary} {:?}", hashes)).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    // env
    dotenvy::dotenv().unwrap();

    let discord_token = match env::var("DISCORD_TOKEN") {
        Ok(v) => v,
        Err(_) => panic!("DISCORD_TOKEN environment variable is missing."),
    };

    let spectre_network_str =
        env::var("SPECTRE_NETWORK").expect("SPECTRE_NETWORK environment variable is missing");

    let wallet_data_path_str =
        env::var("WALLET_DATA_PATH").expect("WALLET_DATA_PATH environment variable is missing");

    // RPC
    let forced_spectre_node: Option<String> = match env::var("FORCE_SPECTRE_NODE_ADDRESS") {
        Ok(v) => Some(v),
        Err(_) => None,
    };

    let resolver = match forced_spectre_node.clone() {
        Some(value) => Resolver::new(Some(vec![Arc::new(value)]), true), // tls
        _ => Resolver::default(),
    };

    let network_id = NetworkId::from_str(&spectre_network_str).unwrap();

    let wrpc_client = Arc::new(
        SpectreRpcClient::new(
            WrpcEncoding::Borsh,
            forced_spectre_node.as_deref(),
            Some(resolver.clone()),
            Some(network_id.clone()),
            None,
        )
        .unwrap(),
    );

    wrpc_client
        .connect(Some(ConnectOptions {
            url: forced_spectre_node.clone(),
            block_async_connect: true,
            ..Default::default()
        }))
        .await
        .unwrap();

    // @TODO(@izio): create the folder if it doesn't exists, on first run it crash otherwise
    let wallet_data_path_buf = BaseDirs::new()
        .unwrap()
        .data_dir()
        .join(wallet_data_path_str.clone());

    let tip_context = TipContext::try_new_arc(
        resolver,
        NetworkId::from_str(&spectre_network_str).unwrap(),
        forced_spectre_node,
        wrpc_client,
        wallet_data_path_buf,
    );

    if let Err(e) = tip_context {
        panic!("{}", format!("Error while building tip context: {}", e));
    }

    // discord
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![wallet()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(tip_context.unwrap())
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged();
    let client = serenity::ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
