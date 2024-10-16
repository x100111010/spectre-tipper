use std::{env, sync::Arc};

use models::tip_context::TipContext;
use poise::serenity_prelude as serenity;
use spectre_wrpc_client::{
    prelude::{NetworkId, NetworkType},
    Resolver,
};

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Arc<TipContext>, Error>;

/// Displays your or another user's account creation date
#[poise::command(slash_command, prefix_command)]
async fn age(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or_else(|| ctx.author());
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

    let tip_context = TipContext::try_new_arc(
        Resolver::default(),
        NetworkId::with_suffix(NetworkType::Testnet, 10),
    )
    .await
    .unwrap();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![age()],
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
