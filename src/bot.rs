use serde::{Deserialize, Serialize};
use std::{panic, str::FromStr, sync::Arc};
use teloxide::{
    dispatching::dialogue::{
        ErasedStorage, GetChatId, InMemStorage, SqliteStorage, Storage, serializer::Json,
    },
    dptree::case,
    payloads::SendMessageSetters,
    prelude::*,
    sugar::bot::BotMessagesExt,
    types::{
        ButtonRequest::Location, InlineKeyboardButton, InlineKeyboardMarkup,
        InlineQueryResultArticle, InputMessageContent, InputMessageContentText, KeyboardButton,
        KeyboardMarkup, KeyboardRemove,
    },
    utils::command::BotCommands,
};
use tzf_rs::DefaultFinder; // Использует встроенные данные

use sqlx::{
    FromRow,
    sqlite::{SqliteConnectOptions, SqliteExecutor, SqlitePool},
};

type MyDialogue = Dialogue<State, ErasedStorage<State>>;
type MyStorage = Arc<ErasedStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, sqlx::FromRow)]
pub struct UserSettings {
    pub chat_id: i64,
    pub timezone: String,
    pub style_path: String,
    pub language: String,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub enum State {
    #[default]
    Start,
    RecieveLanguage,
    RecieveStyle {
        language: String,
    },
    RecieveLocation {
        language: String,
        style_path: String,
    },
    WaitForTask,
    CreateReminder,
}

#[derive(Clone)]
struct Style {
    prompt_path: &'static str,
    premium: bool,
    emoji: &'static str,
}

impl Style {
    fn get_by_emoji(self, text: &str) -> Style {
        if text.contains(self.emoji) {
            return self;
        } else {
            panic!("Trying to find style when there arent");
        }
    }
}

struct StyleDict {
    ru: &'static [&'static str],
    en: &'static [&'static str],
    by: &'static [&'static str],
    styles: &'static [Style],
}

impl StyleDict {
    fn get_vec_by_lang(self, lang: &String) -> Vec<String> {
        let styles_by_lang = match lang.as_str() {
            "ru" => self.ru,
            "en" => self.en,
            "by" => self.by,
            _ => panic!("Unsupported language: {}", lang),
        };

        self.styles
            .iter()
            .zip(styles_by_lang)
            .map(|(style, style_name)| format!("{} {}", style.emoji, style_name))
            .collect()
    }
}

struct Language;

impl Language {
    fn from_str(str: &str) -> String {
        let lang = match str {
            "Русский" => "ru",
            "English" => "en",
            "Беларускі" => "by",
            _ => panic!(
                "Unknown language pass! Pls provide something better than {}",
                str
            ),
        };
        lang.into()
    }

    fn get_vec() -> Vec<String> {
        vec![
            "Русский".to_owned(),
            "English".to_owned(),
            "Беларускі".to_owned(),
        ]
    }

    fn is_supported(text: &str) -> bool {
        Language::get_vec().contains(&text.to_owned())
    }
}

// Не знаю норм ли так оставлять но шо работает то работает
const MAIN_BUTTONS: StyleDict = StyleDict {
    ru: &[
        "Новое напоминание",
        "Настроить Напоминания",
        "Купить премиум",
    ],
    en: &["New reminder", "Manage reminders", "Buy premium"],
    by: &["Новы напамін", "Наладзіць напамінкі", "Купіць прэміум"],
    styles: &[
        Style {
            prompt_path: "new",
            emoji: "🆕",
            premium: false,
        },
        Style {
            prompt_path: "manage",
            emoji: "⚙️",
            premium: false,
        },
        Style {
            prompt_path: "premium",
            emoji: "🌟",
            premium: false,
        },
    ],
};

const STYLES_LANG: StyleDict = StyleDict {
    ru: &["Злой тролль", "Аниме гик", "Церковный"],
    en: &["Angry troll", "Anime geek", "Churchly"],
    by: &["Злы троль", "Анімэ гік", "Царкоўны"],
    styles: &[
        Style {
            prompt_path: "promts/angry_troll.txt",
            emoji: "🤬",
            premium: false,
        },
        Style {
            prompt_path: "prompts/anime_geek.txt",
            emoji: "🇯🇵",
            premium: false,
        },
        Style {
            prompt_path: "prompts/churchly.txt",
            emoji: "🕯️",
            premium: false,
        },
    ],
};

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
                .chain(dptree::filter_map_async(fetch_user_settings_callback))
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
                    case![State::WaitForTask]
                        .chain(dptree::filter_map_async(fetch_user_settings_message))
                        .endpoint(wait_for_task),
                ),
        );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![storage, finder, pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}

async fn fetch_user_settings_callback(pool: SqlitePool, q: CallbackQuery) -> Option<UserSettings> {
    let chat_id = q.from.id.0 as i64;

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

async fn receive_lang(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    if let Some(text) = msg.text() {
        if Language::is_supported(&text.to_owned()) {
            let language = Language::from_str(text);
            let msg_to_send: &str;

            match language.as_str() {
                "ru" => {
                    msg_to_send = "Прекрасно, какой стиль напоминаний предпочитаете?";
                }
                "en" => {
                    msg_to_send = "Nice, what style do you prefer more?";
                }
                "by" => {
                    msg_to_send =
                        "Гэта цудоўна, але, штучны інтэллект патрымлівае нашу мову вельмі дрэнна.
                        \nКарыстуйцеся на сваю рызыку.
                        \nЯкі стыль спадабаецца больш?";
                }
                _ => panic!("Impossible error"),
            }

            let keyboard = make_keyboard(STYLES_LANG.get_vec_by_lang(&language), 3);

            bot.send_message(msg.chat.id, msg_to_send)
                .reply_markup(keyboard)
                .await?;

            dialogue
                .update(State::RecieveStyle {
                    language: language.to_string(),
                })
                .await?;
        } else {
            bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
        }
    } else {
        bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
    }
    Ok(())
}

async fn receive_style(
    bot: Bot,
    dialogue: MyDialogue,
    language: String,
    msg: Message,
) -> HandlerResult {
    match msg.text() {
        Some(style) => {
            let result = STYLES_LANG
                .styles
                .iter()
                .find(|style_elem| style.contains(style_elem.emoji));
            if let Some(result) = result {
                let msg_to_send: &str = match language.as_str() {
                    "ru" => {
                        "Теперь попрошу вас прислать геопозицию для точного присылания напоминаний. Нажмите на кнопочку ниже"
                    }
                    "en" => {
                        "Now I want to ask you geolocation for proper reminder sending. Press the button below"
                    }
                    "by" => {
                        "Цяпер я хачу запрасіць вашу геалакацыю для дакладнага адпраўлення напамінкаў. Націсніце на кнопку ніжэй"
                    }
                    _ => panic!("Impossible error"),
                };

                let geo_button = KeyboardButton::new("📍").request(Location);
                let keyboard = KeyboardMarkup::new(vec![vec![geo_button]])
                    .one_time_keyboard()
                    .resize_keyboard();

                bot.send_message(msg.chat.id, msg_to_send)
                    .reply_markup(keyboard)
                    .await?;

                dialogue
                    .update(State::RecieveLocation {
                        language,
                        style_path: result.prompt_path.to_owned(),
                    })
                    .await?;
            } else {
                bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
            }
        }
        _ => {
            bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
        }
    }

    Ok(())
}

async fn receive_location(
    bot: Bot,
    dialogue: MyDialogue,
    (language, style_path): (String, String),
    msg: Message,
    finder: Arc<DefaultFinder>,
    pool: SqlitePool,
) -> HandlerResult {
    if let Some(location) = msg.location() {
        let lat = location.latitude;
        let lon = location.longitude;
        let timezone = finder.get_tz_name(lon, lat).to_string();
        let msg_to_send = match language.as_str() {
            "ru" => format!(
                "Часовой пояс установлен для {timezone}!\n\nВсё готово! Пользуйтесь кнопками ниже."
            ),
            "en" => format!("Time zone set to {timezone}!\n\nAll done! Use buttons below."),
            "by" => format!(
                "Гадзінны пояс установленны на {timezone}!\n\nУсё гатова! Карыстайцеся кнопкамі ніжэй"
            ),
            _ => panic!("Impossible error"),
        };

        let keyboard = make_keyboard(MAIN_BUTTONS.get_vec_by_lang(&language), 1);

        bot.send_message(msg.chat.id, msg_to_send)
            .reply_markup(keyboard)
            .await?;

        sqlx::query(
            "INSERT INTO users (chat_id, language, style_path, timezone)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT(chat_id) DO UPDATE SET
               language = excluded.language,
               style_path = excluded.style_path,
               timezone = excluded.timezone",
        )
        .bind(msg.chat.id.0)
        .bind(language)
        .bind(style_path)
        .bind(timezone)
        .execute(&pool)
        .await?;

        dialogue.update(State::WaitForTask).await?;
        log::info!("Updating to wait_for_task");
    } else {
        bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
    }

    Ok(())
}

async fn filter_buttons(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    settings: UserSettings,
    state: State,
) -> HandlerResult {
    match state {
        State::WaitForTask => {
            if let Some(data) = q.data.as_ref() {
                let text = format!("{}", settings.language);
                match data.as_str() {
                    "today" => {}
                    _ => {}
                }

                // Tell telegram that we've seen this query, to remove 🕑 icons from the
                // clients. You could also use `answer_callback_query`'s optional
                // parameters to tweak what happens on the client side.
                bot.answer_callback_query(q.id.clone()).await?;

                // Edit text of the message to which the buttons were attached
                if let Some(message) = q.regular_message() {
                    bot.edit_text(message, text).await?;
                } else if let Some(id) = q.inline_message_id {
                    bot.edit_message_text_inline(id, text).await?;
                }

                log::info!("You chose: {data}");
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

                    // Через пару дней
                    //

                    let keyboard = make_inline_keyboard(vector_names, vector_data, 1);
                    bot.send_message(msg.chat.id, msg_to_send)
                        .reply_markup(KeyboardRemove::new())
                        .reply_markup(keyboard)
                        .await?;
                }
                "manage" => {}
                "premium" => {}
                _ => panic!("impossible error"),
            }
        }
    }
    Ok(())
}

// Creates a keyboard made by buttons in a big column.
fn make_inline_keyboard(
    vector_names: Vec<&str>,
    vector_data: Vec<&str>,
    chunks: usize,
) -> InlineKeyboardMarkup {
    let mut keyboard: Vec<Vec<InlineKeyboardButton>> = vec![];

    // 1. Собираем пары (имя, данные) в промежуточный вектор
    let combined: Vec<_> = vector_names.iter().zip(vector_data.iter()).collect();

    // 2. Итерируемся по кускам (строкам клавиатуры)
    for chunk in combined.chunks(chunks) {
        let row = chunk
            .iter()
            .map(|(name, data)| {
                // 3. Создаем кнопку (клонируем строки для callback_data)
                InlineKeyboardButton::callback(name.to_string(), data.to_string())
            })
            .collect::<Vec<InlineKeyboardButton>>();

        // 4. Добавляем строку в общую разметку
        keyboard.push(row);
    }

    InlineKeyboardMarkup::new(keyboard)
}

fn make_keyboard(vector: Vec<String>, chunks: usize) -> KeyboardMarkup {
    let mut keyboard: Vec<Vec<KeyboardButton>> = vec![];

    for elements in vector.chunks(chunks) {
        let row = elements
            .iter()
            .map(|element| KeyboardButton::new(element))
            .collect();

        keyboard.push(row);
    }

    KeyboardMarkup::new(keyboard).resize_keyboard()
    //.one_time_keyboard()
}

// Какие же стили
// Агрессивный тролль
// Аниме гик
// Церковный
