type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

use std::{panic, sync::Arc};
use teloxide::{
    dispatching::dialogue::{GetChatId, InMemStorage},
    payloads::SendMessageSetters,
    prelude::*,
    types::{
        ButtonRequest::Location, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton,
        KeyboardMarkup,
    },
    utils::command::BotCommands,
};
use tzf_rs::DefaultFinder; // Использует встроенные данные

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Start,
    RecieveLanguage,
    RecieveStyle {
        language: &'static str,
    },
    RecieveLocation {
        language: &'static str,
        style: Style,
    },
    WaitForTask {
        language: &'static str,
        style: Style,
        timezone: String,
    },
    // Создать напоминание
    //
}

// /// These commands are supported:
// #[derive(BotCommands)]
// #[command(rename_rule = "lowercase")]
// enum Command {
//     /// Display this text
//     Help,
//     /// Start
//     Start,
// }
// Процес создания напоминаний
// Создать
// Через сколько дней:
// Какого числа:

#[derive(Clone, Copy)]
struct User {
    id: ChatId,
    lang: &'static str,
    style: Style,
    is_premium: bool,
}

#[derive(Clone, Copy)]
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
    fn get_vec_by_lang(self, lang: &str) -> Vec<String> {
        let styles_by_lang = match lang {
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
    fn from_str(str: &str) -> &'static str {
        match str {
            "Русский" => "ru",
            "English" => "en",
            "Беларускі" => "by",
            _ => panic!(
                "Unknown language pass! Pls provide something better than {}",
                str
            ),
        }
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
    log::info!("Starting buttons bot...");

    let bot = Bot::from_env();
    let finder = Arc::new(DefaultFinder::new());

    let handler = dptree::entry().branch(
        Update::filter_message()
            .enter_dialogue::<Message, InMemStorage<State>, State>()
            .branch(dptree::case![State::Start].endpoint(start))
            .branch(dptree::case![State::RecieveLanguage].endpoint(receive_lang))
            .branch(dptree::case![State::RecieveStyle { language }].endpoint(receive_style))
            .branch(
                dptree::case![State::RecieveLocation { language, style }]
                    .endpoint(receive_location),
            )
            .branch(
                dptree::case![State::WaitForTask {
                    language,
                    style,
                    timezone
                }]
                .branch(Update::filter_callback_query().endpoint(filter_buttons))
                .branch(Update::filter_message().endpoint(filter_messages)),
            ),
    );

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![InMemStorage::<State>::new(), finder])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
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
            let language: &str = Language::from_str(text);
            let msg_to_send: &str;

            match language {
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

            let keyboard = make_keyboard(STYLES_LANG.get_vec_by_lang(language), 3);

            bot.send_message(msg.chat.id, msg_to_send)
                .reply_markup(keyboard)
                .await?;

            dialogue
                .update(State::RecieveStyle { language: language })
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
    language: &'static str,
    msg: Message,
) -> HandlerResult {
    match msg.text() {
        Some(style) => {
            let result = STYLES_LANG
                .styles
                .iter()
                .find(|style_elem| style.contains(style_elem.emoji));
            if let Some(result) = result {
                let msg_to_send: &str = match language {
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
                        style: result.to_owned(),
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
    (language, style): (&'static str, Style),
    msg: Message,
    finder: Arc<DefaultFinder>,
) -> HandlerResult {
    if let Some(location) = msg.location() {
        let lat = location.latitude;
        let lon = location.longitude;
        let timezone = finder.get_tz_name(lon, lat).to_string();
        let msg_to_send = match language {
            "ru" => format!("Часовой пояс установлен для {timezone}!"),
            "en" => format!("Time zone set to {timezone}!"),
            "by" => format!("Гадзінны пояс установленны на {timezone}!"),
            _ => panic!("Impossible error "),
        };

        let keyboard = make_keyboard(MAIN_BUTTONS.get_vec_by_lang(language), 1);

        bot.send_message(msg.chat.id, msg_to_send)
            .reply_markup(keyboard)
            .await?;

        dialogue
            .update(State::WaitForTask {
                language,
                style,
                timezone,
            })
            .await?;
    } else {
        bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
    }

    Ok(())
}

async fn wait_for_task(
    bot: Bot,
    dialogue: MyDialogue,
    (language, style, timezone): (&'static str, Style, String),
    msg: Message,
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
                    let msg_to_send: &str = match language {
                        "ru" => "Когда придёт напоминание?",
                        "en" => "When reminder will come to you?",
                        "by" => "Калі прыдзе ваш напамінак?",
                        _ => panic!("impossible error"),
                    };

                    bot.send_message(msg.chat.id, msg_to_send)
                        //.reply_markup()
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

async fn filter_buttons(
    bot: Bot,
    dialogue: MyDialogue,
    (language, style, timezone): (&'static str, Style, String),
    msg: Message,
) -> HandlerResult {
    Ok(())
}

async fn filter_messages(
    bot: Bot,
    dialogue: MyDialogue,
    (language, style, timezone): (&'static str, Style, String),
    msg: Message,
) -> HandlerResult {
    if let Some(text) = msg.text() {
        bot.send_message(msg.chat.id, "succes numba2").await?;
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

    for elements in vector_names.chunks(chunks) {
        let row = elements
            .iter()
            .zip(&vector_data)
            .map(|(name, data)| InlineKeyboardButton::callback(name.to_owned(), data.to_owned()))
            .collect();

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
