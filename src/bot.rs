use serde::{Deserialize, Serialize};
use std::{panic, str::FromStr, sync::Arc};
use teloxide::{
    dispatching::dialogue::{ErasedStorage, SqliteStorage, Storage, serializer::Json},
    dptree::case,
    payloads::SendMessageSetters,
    prelude::*,
    sugar::bot::BotMessagesExt,
    types::KeyboardRemove,
};
use tzf_rs::DefaultFinder; // Использует встроенные данные

use sqlx::{
    FromRow,
    sqlite::{SqliteConnectOptions, SqlitePool},
};

use crate::{
    get_user_info::{Language, MAIN_BUTTONS, receive_lang, receive_location, receive_style},
    utils::{make_inline_keyboard, make_keyboard},
};

pub type MyDialogue = Dialogue<State, ErasedStorage<State>>;
pub type MyStorage = Arc<ErasedStorage<State>>;
pub type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct UserSettings {
    pub timezone: String,
    pub style_path: String,
    pub language: String,
    pub chat_id: i64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub enum State {
    #[default]
    // Gettings user info
    Start,
    RecieveLanguage,
    RecieveStyle {
        language: String,
    },
    RecieveLocation {
        language: String,
        style_path: String,
    },

    WaitForTask {
        user_settings: UserSettings,
    }, // Waiting for user to select a task

    // Creating reminder
    CreateReminder {
        user_settings: UserSettings,
    },
    RecieveReminderDate {
        user_settings: UserSettings,
        exact_date: bool,
    },
    RecieveReminderTime {
        user_settings: UserSettings,
        date: String,
    },
    RecieveAdditionalInfo {
        user_settings: UserSettings,
        date: String,
        time: String,
    },
}

pub async fn run() {
    pretty_env_logger::init();
    log::info!("Starting bot...");

    let bot = Bot::from_env();
    let finder = Arc::new(DefaultFinder::new());

    let options = SqliteConnectOptions::from_str("sqlite://databases/users_info.db")
        .expect("SQLite connection options failed")
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options)
        .await
        .expect("Pool didnt created successfully");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
                chat_id INTEGER PRIMARY KEY,
                language TEXT NOT NULL,
                style_path TEXT NOT NULL,
                timezone TEXT NOT NULL
            )",
    )
    .execute(&pool)
    .await
    .expect("Coudnt create db table");

    let storage: MyStorage = SqliteStorage::open("databases/users_state.db", Json)
        .await
        .expect("cant open users_state.db")
        .erase();

    let handler = dptree::entry()
        .branch(
            Update::filter_callback_query()
                .enter_dialogue::<CallbackQuery, ErasedStorage<State>, State>()
                //.chain(dptree::filter_map_async(fetch_user_settings_callback))
                .endpoint(filter_buttons),
        )
        .branch(
            Update::filter_message()
                .enter_dialogue::<Message, ErasedStorage<State>, State>()
                .branch(case![State::Start].endpoint(start))
                .branch(case![State::RecieveLanguage].endpoint(receive_lang))
                .branch(case![State::RecieveStyle { language }].endpoint(receive_style))
                .branch(
                    case![State::RecieveLocation {
                        language,
                        style_path
                    }]
                    .endpoint(receive_location),
                )
                .branch(
                    case![State::WaitForTask { user_settings }]
                        //.chain(dptree::filter_map_async(fetch_user_settings_message))
                        .endpoint(wait_for_task),
                )
                .branch(case![State::CreateReminder { user_settings }].endpoint(create_reminder)),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, finder, pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

// async fn fetch_user_settings_callback(pool: SqlitePool, q: CallbackQuery) -> Option<UserSettings> {
//     let chat_id = q.from.id.0 as i64;

//     let settings_row = sqlx::query("SELECT * FROM users WHERE chat_id = ?")
//         .bind(chat_id)
//         .fetch_one(&pool)
//         .await;

//     match settings_row {
//         Ok(row) => {
//             log::info!("got user settings for chat_id: {}", chat_id);
//             Some(UserSettings::from_row(&row).expect("coudnt take user settings from row"))
//         }
//         Err(_) => None,
//     }
// }

async fn fetch_user_settings_message(
    pool: SqlitePool,
    dialogue: MyDialogue,
) -> Option<UserSettings> {
    let chat_id = dialogue.chat_id().0;

    let settings_row = sqlx::query("SELECT * FROM users WHERE chat_id = ?")
        .bind(chat_id)
        .fetch_one(&pool)
        .await;

    match settings_row {
        Ok(row) => {
            log::info!("got user settings for chat_id: {}", chat_id);
            Some(UserSettings::from_row(&row).expect("coudnt take user settings from row"))
        }
        Err(_) => None,
    }
}

async fn start(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let keyboard = make_keyboard(Language::get_vec(), 3);
    bot.send_message(msg.chat.id, "🗣️❓")
        .reply_markup(keyboard)
        .await?;
    dialogue.update(State::RecieveLanguage).await?;
    Ok(())
}

async fn filter_buttons(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    state: State,
) -> HandlerResult {
    match state {
        State::CreateReminder { user_settings } => {
            if let Some(data) = q.data.as_ref() {
                match data.as_str() {
                    "today" => {
                        let text = match user_settings.language.as_str() {
                            "en" => {
                                "Write your reminder time in the format Hour:Minute (only 24-hour format allowed!)"
                            }
                            "ru" => "Напишите время для напоминания в виде Час:Минута",
                            "by" => "Напішыце час для напамінка ў фармату Гадзіна:Мінута",
                            _ => "This should not happen, please report this issue",
                        };

                        if let Some(message) = q.regular_message() {
                            bot.edit_text(message, text).await?;
                        } else if let Some(id) = q.inline_message_id {
                            bot.edit_message_text_inline(id, text).await?;
                        }

                        dialogue
                            .update(State::RecieveReminderTime {
                                user_settings: user_settings,
                                date: chrono::Local::now().date_naive().to_string(),
                            })
                            .await?
                    }
                    "other_day" => {
                        let text = match user_settings.language.as_str() {
                            "en" => "Write in how many days the reminder will arrive.",
                            "ru" => "Напишите через сколько дней должно прийти напоминание.",
                            "by" => "Напішыце праз колькі дзён прыдзе напамінак.",
                            _ => "This should not happen, please report this issue",
                        };

                        if let Some(message) = q.regular_message() {
                            bot.edit_text(message, text).await?;
                        } else if let Some(id) = q.inline_message_id {
                            bot.edit_message_text_inline(id, text).await?;
                        }

                        dialogue
                            .update(State::RecieveReminderDate {
                                user_settings: user_settings,
                                exact_date: false,
                            })
                            .await?
                    }
                    "some_day" => {
                        let text = "Напиши дату в формате день";
                        if let Some(message) = q.regular_message() {
                            bot.edit_text(message, text).await?;
                        } else if let Some(id) = q.inline_message_id {
                            bot.edit_message_text_inline(id, text).await?;
                        }

                        dialogue
                            .update(State::RecieveReminderDate {
                                user_settings: user_settings,
                                exact_date: true,
                            })
                            .await?
                    }
                    _ => {}
                }

                bot.answer_callback_query(q.id.clone()).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn wait_for_task(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    settings: UserSettings,
) -> HandlerResult {
    if let Some(text) = msg.text() {
        let button = MAIN_BUTTONS
            .styles
            .iter()
            .find(|style| text.contains(style.emoji));

        if let Some(button) = button {
            match button.prompt_path {
                // На самом деле это не промпт а просто действия, не хочу сравнивать по эмодзи, это не удобно
                "new" => {
                    let msg_to_send: &str;
                    let vector_names: Vec<&str>;
                    let vector_data: Vec<&str> = vec!["today", "other_day", "some_date"];
                    match settings.language.as_str() {
                        "ru" => {
                            msg_to_send = "Когда придёт напоминание?";
                            vector_names = vec!["Сегодня", "Через пару дней", "Какого-то числа"]
                        }
                        "en" => {
                            msg_to_send = "When reminder will come to you?";
                            vector_names = vec!["Today", "Days later", "In some exact date"]
                        }
                        "by" => {
                            msg_to_send = "Калі прыдзе ваш напамінак?";
                            vector_names = vec!["Сягодня", "Праз пару дзён", "Нейкага чысла"]
                        }
                        _ => panic!("impossible error"),
                    };

                    let keyboard = make_inline_keyboard(vector_names, vector_data, 1);
                    bot.send_message(msg.chat.id, msg_to_send)
                        .reply_markup(keyboard)
                        .await?;

                    dialogue
                        .update(State::CreateReminder {
                            user_settings: settings,
                        })
                        .await?
                }
                "manage" => {}
                "premium" => {}
                _ => panic!("impossible error"),
            }
        }
    }
    Ok(())
}

async fn recieve_reminder_time(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    user_settings: UserSettings,
    date: String,
) -> HandlerResult {
    if let Some(text) = msg.text() {}
    Ok(())
}

async fn create_reminder(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    user_settings: UserSettings,
) -> HandlerResult {
    Ok(())
}

async fn recieve_reminder_date(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    user_settings: UserSettings,
    exact_date: bool,
) -> HandlerResult {
    if let Some(text) = msg.text() {
        if exact_date {
        } else {
        }
    }
    Ok(())
}

// Какие же стили
// Агрессивный тролль
// Аниме гик
// Церковный
