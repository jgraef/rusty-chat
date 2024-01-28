use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::Write,
};

use chrono::{
    DateTime,
    Local,
};
use leptos::{
    Signal,
    WriteSignal,
};
use leptos_use::storage::{
    use_local_storage,
    JsonCodec,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum StorageKey {
    State,
    Message(MessageId),
}

impl StorageKey {
    fn as_str(&self) -> Cow<'static, str> {
        match self {
            Self::State => "state".into(),
            Self::Message(id) => format!("msg-{id}").into(),
        }
    }
}

pub fn use_storage<T: Serialize + for<'de> Deserialize<'de> + Clone + Default + PartialEq>(
    key: StorageKey,
) -> (Signal<T>, WriteSignal<T>, impl Fn() + Clone) {
    let key = key.as_str();
    use_local_storage::<T, JsonCodec>(key)
}

pub fn use_message(
    id: MessageId,
) -> (
    Signal<Option<Message>>,
    WriteSignal<Option<Message>>,
    impl Fn() + Clone,
) {
    use_storage::<Option<Message>>(StorageKey::Message(id))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default = "current_state_version")]
    pub version: u32,
    pub models: BTreeMap<ModelId, Model>,
    pub conversations: BTreeMap<ConversationId, Conversation>,
    pub current_model: ModelId,
    pub current_system_prompt: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        let mut models = BTreeMap::new();

        let mut add_model = |model_id: &'static str, chat_template| {
            let model_id = ModelId(model_id.to_owned());
            models.insert(
                model_id.clone(),
                Model {
                    model_id,
                    chat_template,
                },
            )
        };
        add_model(
            "NousResearch/Nous-Hermes-2-Mixtral-8x7B-DPO",
            ChatTemplate::ChatML,
        );
        add_model("mistralai/Mistral-7B-Instruct-v0.2", ChatTemplate::Instruct);

        #[derive(Debug, Deserialize)]
        struct DefaultModels {
            model: Vec<Model>,
        }
        let default_models: DefaultModels =
            toml::from_str(include_str!("../default_models.toml")).unwrap();
        for model in default_models.model {
            models.insert(model.model_id.clone(), model);
        }

        let current_model = models
            .first_key_value()
            .expect("expected at least one model")
            .0
            .to_owned();

        Self {
            version: current_state_version(),
            models,
            conversations: BTreeMap::new(),
            current_model,
            current_system_prompt: None,
        }
    }
}

fn current_state_version() -> u32 {
    1
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

impl From<&str> for ModelId {
    fn from(value: &str) -> Self {
        ModelId(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub model_id: ModelId,
    pub chat_template: ChatTemplate,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ChatTemplate {
    None,
    Instruct,
    ChatML,
}

impl ChatTemplate {
    pub fn supports_system_prompt(&self) -> bool {
        match self {
            ChatTemplate::ChatML => true,
            _ => false,
        }
    }

    pub fn generate_prompt(&self, system_prompt: Option<&str>, messages: &[Message]) -> String {
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
pub struct ConversationId(Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub model_id: ModelId,
    pub hyper_parameters: HyperParameters,
    pub system_prompt: Option<String>,
    pub title: Option<String>,
    pub timestamp_started: DateTime<Local>,
    pub timestamp_last_interaction: DateTime<Local>,
    pub messages: Vec<MessageId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HyperParameters {
    pub temperature: f32,
}

impl Default for HyperParameters {
    fn default() -> Self {
        Self { temperature: 1.0 }
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
