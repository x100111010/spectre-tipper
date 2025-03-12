use core::tip_context::TipContext;
use std::sync::Arc;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::ApplicationContext<'a, Arc<TipContext>, Error>;
