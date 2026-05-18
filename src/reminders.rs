use chrono::Utc;
use chrono_tz::Tz;
use log::log;
use reqwest::{
    Client,
    header::{self, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde::Deserialize;
use serde_json::json;
use sqlx::{SqlitePool, error::BoxDynError};
use teloxide::{ApiError, Bot, RequestError, prelude::Requester, types::ChatId};
use tokio::fs;

#[derive(Clone, sqlx::FromRow)]
struct ReminderQuery {
    pub id: i64,
    pub chat_id: i64,
    pub language: String,
    pub style_path: String,
    pub what_to_remind: String,
    pub date_time: String,
    pub is_premium: bool,
}

#[derive(Clone, sqlx::FromRow)]
pub struct Reminder {
    pub id: i64,
    pub chat_id: i64,
    pub date_time: String,
    pub reminder_text: String,
    pub reminder_agreement: String,
    pub is_sent: bool,
}

#[derive(Deserialize)]
struct ReminderGenerated {
    agreement: String,
    reminder: String,
}

pub async fn generate_reminder(
    reminder_pool: &SqlitePool,
    client: &Client,
) -> Result<(), BoxDynError> {
    match sqlx::query_as::<_, ReminderQuery>(
        "
        SELECT *
        FROM reminder_query
        ORDER BY abs(unixepoch('now') - unixepoch(date_time))
    ",
    )
    .fetch_all(reminder_pool)
    .await
    {
        Ok(reminders) if reminders.len() > 0 => {
            log::info!("Succesfuly got reminders, size: {}", reminders.len());
            for reminder in reminders {
                log::info!("Generating reminder for chat_id: {}", reminder.chat_id);

                let schema = json!({
                    "type": "object",
                    "properties": {
                        "reminder": {
                            "type": "string",
                            "description": "The aggressive troll message, written in {language}, demanding the user to complete {what_to_remind}, and containing the required agreement phrase."
                                .replace("{language}", &reminder.language)
                                .replace("{what_to_remind}", &reminder.what_to_remind)
                        },
                        "agreement": {
                            "type": "string",
                            "description": "The exact humiliating surrender phrase the user must write back, in {language}."
                                .replace("{language}", &reminder.language)
                        }
                    },
                    "required": ["reminder", "agreement"],
                    "additionalProperties": false
                });

                let api_key =
                    std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");

                let url = "https://openrouter.ai/api/v1/chat/completions";

                let prompt = fs::read_to_string(&reminder.style_path)
                    .await?
                    .replace("{what_to_remind}", &reminder.what_to_remind)
                    .replace("{language}", &reminder.language);

                let messages = vec![
                    json!({"role": "system", "content": &prompt}),
                    json!({"role": "user", "content": "Generate the reminder JSON."}),
                ];

                let payload = json!({
                    "model": "deepseek/deepseek-v4-flash",
                    "messages": messages,
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {
                            "name": "reminder_response",
                            "schema": schema,
                        }
                    },
                    "temperature": 0.8,
                    "reasoning": {"enabled": true}
                });

                let client = Client::new();

                let response = client
                    .post(url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await?;

                let assistant_message = &response["choices"][0]["message"];

                let content = assistant_message["content"].as_str().unwrap_or_else(|| {
                    log::error!(
                        "assistang message content error, content: {}\n\nresponce: {}",
                        assistant_message,
                        &response
                    );
                    return "";
                });

                // Парсим сгенерированный JSON
                match serde_json::from_str::<ReminderGenerated>(content) {
                    Ok(reminder_generated) => {
                        sqlx::query(
                            "
                            INSERT INTO reminders (chat_id, date_time, reminder_text, reminder_agreement, is_sent)
                            VALUES ($1, $2, $3, $4, 0)
                            ",
                        )
                        .bind(reminder.chat_id)
                        .bind(reminder.date_time)
                        .bind(reminder_generated.reminder)
                        .bind(reminder_generated.agreement)
                        .execute(reminder_pool)
                        .await?;

                        log::info!("Succesfully saved reminder!");

                        sqlx::query("DELETE FROM reminder_query WHERE id = $1")
                            .bind(reminder.id)
                            .execute(reminder_pool)
                            .await?;
                    }
                    Err(err) => {
                        log::error!("cant parse json, err: {}, json: {}", err, content);
                    }
                }
            }
        }
        Ok(_) => {
            log::info!("Zero reminders to generate, waiting for 1 min");
        }

        Err(err) => {
            log::error!("Error occured when tried to get reminders!, err: {}", err);
        }
    }
    tokio::time::sleep(std::time::Duration::from_mins(1)).await;
    Ok(())
}

pub async fn send_reminders(
    reminders_pool: &SqlitePool,
    bot: Bot,
) -> Result<(), Box<dyn std::error::Error>> {
    match sqlx::query_as::<_, Reminder>(
        "
            SELECT *
            FROM reminders
            WHERE unixepoch(date_time) <= unixepoch('now');
        ",
    )
    .fetch_all(reminders_pool)
    .await
    {
        Ok(prepeared_reminders) if prepeared_reminders.len() > 0 => {
            for reminder in prepeared_reminders {
                if reminder.is_sent == false {
                    sqlx::query(
                        "
                            UPDATE reminders
                            SET is_sent = 1
                            WHERE id = $1;
                        ",
                    )
                    .bind(reminder.id)
                    .fetch_all(reminders_pool)
                    .await?;
                }
                match bot
                    .send_message(ChatId(reminder.chat_id), &reminder.reminder_text)
                    .await
                {
                    Ok(_) => {}
                    Err(err) => {
                        if let RequestError::Api(ApiError::BotBlocked) = err {
                            sqlx::query("DELETE FROM reminders WHERE chat_id = $1")
                                .bind(reminder.chat_id)
                                .execute(reminders_pool)
                                .await?;
                        }
                    }
                }
            }
        }
        Ok(_) => {
            log::info!("No reminders to send, waiting 1 minute");
        }
        Err(err) => {
            log::error!("Coudnt get reminders to send! err: {}", err)
        }
    }
    tokio::time::sleep(std::time::Duration::from_mins(1)).await;
    Ok(())
}
