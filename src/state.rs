#![allow(dead_code)] // backend stuff is not used anywhere yet.

use std::{
    borrow::Cow,
    collections::{
        BTreeMap,
        HashSet,
    },
    fmt::Write,
};

use chrono::{
    DateTime,
    Local,
};
use lazy_static::lazy_static;
use leptos::{
    Signal,
    WriteSignal,
};
use leptos_use::{
    storage::use_local_storage,
    utils::JsonCodec,
};
use semver::Version;
use serde::{
    Deserialize,
    Serialize,
};
use strum::{
    AsRefStr,
    EnumMessage,
    EnumString,
    VariantArray,
};
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum StorageKey {
    Version,
    Home,
    Settings,
    Conversations,
    Conversation(ConversationId),
    Message(MessageId),
}

impl StorageKey {
    fn as_str(&self) -> Cow<'static, str> {
        match self {
            Self::Version => "version".into(),
            Self::Home => "home".into(),
            Self::Settings => "settings".into(),
            Self::Conversations => "conversations".into(),
            Self::Conversation(id) => format!("conversation-{id}").into(),
            Self::Message(id) => format!("message-{id}").into(),
        }
    }
}

#[derive(Clone)]
pub struct StorageSignals<T: 'static> {
    pub key: StorageKey,
    pub read: Signal<T>,
    pub write: WriteSignal<T>,
}

impl<T: 'static> StorageSignals<T> {
    pub fn delete(&self) {
        delete_storage(self.key)
    }
}

pub fn use_storage<T: Serialize + for<'de> Deserialize<'de> + Clone + Default + PartialEq>(
    key: StorageKey,
) -> StorageSignals<T> {
    let (read, write, _) = use_local_storage::<T, JsonCodec>(key.as_str());
    StorageSignals { key, read, write }
}

pub fn clear_storage() {
    let Some(window) = web_sys::window()
    else {
        return;
    };
    let Some(storage) = window.local_storage().ok().flatten()
    else {
        return;
    };
    storage.clear().ok();
}

pub fn delete_storage(key: StorageKey) {
    let Some(window) = web_sys::window()
    else {
        return;
    };
    let Some(storage) = window.local_storage().ok().flatten()
    else {
        return;
    };
    storage.delete(&key.as_str()).ok();
}

pub fn use_version() -> StorageSignals<Option<Version>> {
    use_storage(StorageKey::Version)
}

pub fn use_home() -> StorageSignals<Home> {
    use_storage(StorageKey::Home)
}

pub fn use_settings() -> StorageSignals<Settings> {
    use_storage(StorageKey::Settings)
}

pub fn use_conversations() -> StorageSignals<HashSet<ConversationId>> {
    use_storage(StorageKey::Conversations)
}

pub fn use_conversation(id: ConversationId) -> StorageSignals<Option<Conversation>> {
    use_storage(StorageKey::Conversation(id))
}

pub fn use_message(id: MessageId) -> StorageSignals<Option<Message>> {
    use_storage(StorageKey::Message(id))
}

#[derive(Debug, Deserialize)]
struct DefaultSettings {
    pub default_model: Option<ModelId>,
    #[serde(rename = "model", default)]
    models: Vec<Model>,
}

lazy_static! {
    static ref DEFAULT_SETTINGS: DefaultSettings =
        toml::from_str(include_str!("../default_settings.toml"))
            .expect("invalid default_settings.toml");
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub models: BTreeMap<ModelId, Model>,
    pub debug_mode: bool,
}

impl Settings {
    pub fn reset(&mut self) {
        self.models = DEFAULT_SETTINGS
            .models
            .iter()
            .map(|model| (model.model_id.clone(), model.clone()))
            .collect();
    }
}

impl Default for Settings {
    fn default() -> Self {
        let mut this = Self {
            models: BTreeMap::new(),
            debug_mode: false,
        };
        this.reset();
        this
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Home {
    pub selected_model: Option<ModelId>,
    #[serde(default)]
    pub conversation_parameters: ConversationParameters,
    #[serde(default)]
    pub user_message: String,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            selected_model: default_model(),
            conversation_parameters: Default::default(),
            user_message: Default::default(),
        }
    }
}

fn default_model() -> Option<ModelId> {
    DEFAULT_SETTINGS.default_model.clone()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]

pub struct ConversationParameters {
    pub system_prompt: Option<String>,
    pub start_response_with: Option<String>,
    pub token_limit: Option<usize>,
    pub temperature: Option<f32>,
    pub top_k: Option<usize>,
    pub top_p: Option<f32>,
    pub repetition_penalty: Option<f32>,
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::From,
)]
#[serde(transparent)]
pub struct ConversationId(Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub model_id: Option<ModelId>,
    pub title: Option<String>,
    pub timestamp_started: DateTime<Local>,
    pub timestamp_last_interaction: DateTime<Local>,
    pub conversation_parameters: ConversationParameters,
    pub user_message: String,
    pub messages: Vec<MessageId>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::From,
)]
#[serde(transparent)]
pub struct ModelId(pub String);

impl ModelId {
    pub fn url(&self) -> String {
        format!("https://huggingface.co/{}", self.0)
    }
}

impl From<&str> for ModelId {
    fn from(value: &str) -> Self {
        ModelId(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub model_id: ModelId,
    pub name: Option<String>,
    #[serde(default)]
    pub chat_template: ChatTemplate,
}

impl Model {
    pub fn display_name(&self) -> &str {
        self.name.as_ref().unwrap_or_else(|| &self.model_id.0)
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    VariantArray,
    EnumString,
    AsRefStr,
    EnumMessage,
)]
pub enum ChatTemplate {
    #[strum(message = "None")]
    None,
    #[strum(message = "Instruct")]
    Instruct,
    #[strum(message = "ChatML")]
    ChatML,
}

impl Default for ChatTemplate {
    fn default() -> Self {
        Self::None
    }
}

impl ChatTemplate {
    pub fn supports_system_prompt(&self) -> bool {
        match self {
            ChatTemplate::ChatML => true,
            _ => false,
        }
    }

    pub fn generate_prompt(
        &self,
        system_prompt: Option<&str>,
        messages: &[Message],
        start_response_with: Option<&str>,
    ) -> String {
        let mut prompt = String::new();
        match self {
            Self::None => {
                for message in messages {
                    write!(&mut prompt, "{}\n", message.text).unwrap();
                }
            }
            Self::Instruct => {
                for message in messages {
                    if matches!(message.role, Role::User) {
                        write!(&mut prompt, "[INST] {} [/INST]\n", message.text).unwrap();
                    }
                    else {
                        write!(&mut prompt, "{}\n", message.text).unwrap();
                    }
                }
            }
            Self::ChatML => {
                if let Some(system_prompt) = system_prompt {
                    write!(
                        &mut prompt,
                        "<|im_start|>system\n{system_prompt}<|im_end|>\n"
                    )
                    .unwrap();
                }
                for message in messages {
                    let role = match message.role {
                        Role::Assitant => "assistant",
                        Role::User => "user",
                    };
                    write!(
                        &mut prompt,
                        "<|im_start|>{role}\n{}<|im_end|>\n",
                        message.text
                    )
                    .unwrap();
                }
                write!(&mut prompt, "<|im_start|>assistant\n").unwrap()
            }
        }
        if let Some(start_response_with) = start_response_with {
            prompt.push_str(start_response_with);
        }
        prompt
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::From,
)]
#[serde(transparent)]
pub struct MessageId(Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub text: String,
    pub timestamp: DateTime<Local>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Assitant,
    User,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BackendKind {
    HuggingFace,
    LlamaCpp,
    LlamaCppRs,
}

impl BackendKind {
    // todo: differences in how models can be selected
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]

pub struct BackendSettings {
    id: Uuid,
    name: String,
    settings: BackendKindSettings,
}

impl BackendSettings {
    pub fn kind(&self) -> BackendKind {
        self.settings.kind()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]

pub enum BackendKindSettings {
    HuggingFace { hf_token: Option<String> },
    LlamaCpp { url: String },
    LlamaCppRs { url: String, token: Option<String> },
}

impl BackendKindSettings {
    pub fn kind(&self) -> BackendKind {
        match self {
            BackendKindSettings::HuggingFace { .. } => BackendKind::HuggingFace,
            BackendKindSettings::LlamaCpp { .. } => BackendKind::LlamaCpp,
            BackendKindSettings::LlamaCppRs { .. } => BackendKind::LlamaCppRs,
        }
    }
}
