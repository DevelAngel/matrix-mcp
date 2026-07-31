use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};

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

    pub async fn send(&self, system_msg: &str, user_msg: &str) -> Result<String> {
        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(system_msg)
            .build()
            .context("failed to create system message")?;

        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(user_msg)
            .build()
            .context("failed to create user message")?;

        let request = CreateChatCompletionRequestArgs::default()
            .model("auto")
            .messages(vec![system_msg.into(), user_msg.into()])
            .temperature(0.7)
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
