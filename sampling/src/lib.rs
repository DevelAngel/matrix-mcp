use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};

pub enum Message {
    User(String),
    Assistant(String),
}

pub struct LLM {
    client: Client<OpenAIConfig>,
}

impl LLM {
    pub fn connect(api_base_url: &str, api_key: Option<&str>) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(api_base_url)
            .with_api_key(api_key.unwrap_or("not-needed"));
        let client = Client::with_config(config);
        Self { client }
    }

    pub async fn send(
        &self,
        system_prompt: Option<&str>,
        messages: Vec<Message>,
        temperature: f32,
    ) -> Result<String> {
        let mut chat_messages = Vec::<ChatCompletionRequestMessage>::with_capacity(
            messages.len() + system_prompt.map_or(0, |_| 1),
        );
        if let Some(system_prompt) = system_prompt {
            let system_prompt = ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()
                .context("failed to create system message")?;
            chat_messages.push(system_prompt.into());
        }
        for msg in messages {
            let msg = match msg {
                Message::User(msg) => {
                    let msg = ChatCompletionRequestUserMessageArgs::default()
                        .content(msg)
                        .build()
                        .context("failed to create user message")?;
                    msg.into()
                }
                Message::Assistant(msg) => {
                    let msg = ChatCompletionRequestAssistantMessageArgs::default()
                        .content(msg)
                        .build()
                        .context("failed to create user message")?;
                    msg.into()
                }
            };
            chat_messages.push(msg);
        }

        let request = CreateChatCompletionRequestArgs::default()
            .model("auto")
            .messages(chat_messages)
            .temperature(temperature)
            .build()?;
        tracing::warn!("send message to LLM, answer may take some mintues");
        let response = &self
            .client
            .chat()
            .create(request)
            .await
            .context("failed to communicate with LLM")?;
        tracing::info!("received answer from LLM");
        tracing::debug!("LLM response:\n{response:?}");

        let text = response.choices[0]
            .message
            .content
            .clone()
            .context("failed to parse response message")?;
        Ok(text)
    }
}
