use std::{env, sync::Arc};

use core::{error::Error as SpectreError, tip_context::TipContext, tip_wallet::TipOwnedWallet};
use poise::{
    serenity_prelude::{self as serenity, Colour, CreateEmbed},
    CreateReply, Modal,
};
use spectre_wallet_core::prelude::{Language, Mnemonic};
use spectre_wallet_keys::secret::Secret;
use spectre_wrpc_client::{
    prelude::{NetworkId, NetworkType},
    Resolver,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;

// TODO: mutualize embed creation (avoid repetition and centralize calls) and reply in general
// TODO: move cmd to dedicated files

#[poise::command(
    slash_command,
    subcommands("create", "open", "restore", "status", "destroy"),
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
    }

    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_open_wallet_exists(&wallet_owner_identifier);
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
        ctx.say(format!("a discord wallet already exists",)).await?;

        return Ok(());
    }

    let (tip_wallet, mnemonic) = TipOwnedWallet::create(
        tip_context.clone(),
        &Secret::from(secret),
        &wallet_owner_identifier,
    )
    .await?;

    ctx.say(format!(
        "{}\n{}",
        mnemonic.phrase(),
        tip_wallet.receive_address()
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// open the discord wallet using the secret
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
    if let Some(wallet) = tip_context.get_open_wallet_arc(&wallet_owner_identifier) {
        ctx.say(format!("{}", wallet.receive_address())).await?;

        return Ok(());
    }

    // TODO: add check a wallet doesn't exist yet
    //       else ask remove first (needs implementation, warning: a wallet can be opened need to be closed first)

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

    ctx.say(format!("{}", tip_wallet.receive_address())).await?;

    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// get the status of your discord wallet
async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_open_wallet_exists(&wallet_owner_identifier);
    let is_initiated = match is_opened {
        true => true,
        false => {
            tip_context
                .local_store()?
                .exists(Some(&wallet_owner_identifier))
                .await?
        }
    };

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
    #[name = "write detroy to confirm"]
    first_input: String,
}

#[poise::command(slash_command, category = "wallet")]
/// destroy your existing (if exists) discord wallet
async fn destroy(ctx: Context<'_>) -> Result<(), Error> {
    let user = ctx.author().id;
    let wallet_owner_identifier = user.to_string();

    let tip_context = ctx.data();

    let is_opened = tip_context.does_open_wallet_exists(&wallet_owner_identifier);
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
        ctx.say(format!(
            "the wallet is not initiated, cannot destroy a non existing thing"
        ))
        .await?;

        return Ok(());
    }

    let result = DestructionModalConfirmation::execute(ctx).await?;

    if let Some(data) = result {
        if data.first_input == "destroy" {
            if is_opened {
                let tip_wallet_result = tip_context.remove_opened_wallet(&wallet_owner_identifier);

                if let Some(tip_wallet) = tip_wallet_result {
                    tip_wallet.wallet().close().await?;
                };
            }

            // TODO: erase the file on file system, current storage implementation disallow this via direct API access

            ctx.say(format!("destroy ok")).await?;

            return Ok(());
        }
    }

    ctx.say(format!("destroy aborted")).await?;

    return Ok(());
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

    ctx.say(recovered_tip_wallet_result.receive_address())
        .await?;
    Ok(())
}

#[poise::command(slash_command, category = "wallet")]
/// send to user the given amount
async fn send(
    ctx: Context<'_>,
    #[description = "Send to"] user: serenity::User,
    #[description = "Amount"] amount: i64,
) -> Result<(), Error> {
    let u = user;
    let response = format!("{}'s account was created at {}", u.name, u.created_at());
    ctx.say(response).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().unwrap();

    let discord_token = match env::var("DISCORD_TOKEN") {
        Ok(v) => v,
        Err(_) => panic!("DISCORD_TOKEN environment variable is missing."),
    };

    let intents = serenity::GatewayIntents::non_privileged();

    let tip_context = TipContext::new_arc(
        Resolver::default(),
        NetworkId::with_suffix(NetworkType::Testnet, 10),
    );

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![wallet()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(tip_context)
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
