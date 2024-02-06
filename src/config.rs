use lazy_static::lazy_static;
use serde::Deserialize;

use crate::state::{
    Model,
    ModelId,
};

pub const GITHUB_PAGE: &'static str = "https://github.com/jgraef/rusty-chat/";
pub const GITHUB_ISSUES_PAGE: &'static str = "https://github.com/jgraef/rusty-chat/issues";

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub examples: Vec<String>,
    pub default_model: ModelId,
    #[serde(rename = "model", default)]
    pub models: Vec<Model>,
}

lazy_static! {
    pub static ref BUILD_CONFIG: BuildConfig =
        toml::from_str(include_str!("../config.toml")).expect("invalid config.toml");
}
