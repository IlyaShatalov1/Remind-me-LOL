use std::sync::Arc;

use sqlx::SqlitePool;
use teloxide::{
    Bot,
    payloads::SendMessageSetters,
    prelude::Requester,
    types::{ButtonRequest::Location, KeyboardButton, KeyboardMarkup, Message},
};
use tzf_rs::DefaultFinder;

use crate::bot::{HandlerResult, MyDialogue, State};
use crate::utils::make_keyboard;

#[derive(Clone)]
pub struct Style {
    pub prompt_path: &'static str,
    pub premium: bool,
    pub emoji: &'static str,
}

impl Style {
    pub fn get_by_emoji(self, text: &str) -> Style {
        if text.contains(self.emoji) {
            self
        } else {
            panic!("Trying to find style when there arent");
        }
    }
}

pub struct StyleDict {
    pub ru: &'static [&'static str],
    pub en: &'static [&'static str],
    pub by: &'static [&'static str],
    pub styles: &'static [Style],
}

impl StyleDict {
    pub fn get_vec_by_lang(self, lang: &String) -> Vec<String> {
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

pub struct Language;

impl Language {
    pub fn from_str(str: &str) -> String {
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

    pub fn get_vec() -> Vec<String> {
        vec![
            "Русский".to_owned(),
            "English".to_owned(),
            "Беларускі".to_owned(),
        ]
    }

    pub fn is_supported(text: &str) -> bool {
        Language::get_vec().contains(&text.to_owned())
    }
}

pub const STYLES_LANG: StyleDict = StyleDict {
    ru: &["Злой тролль"], // "Аниме гик", "Церковный" TODO: Добавить в будущем
    en: &["Angry troll"], // "Anime geek", "Churchly"
    by: &["Злы троль"],   // "Анімэ гік", "Царкоўны"
    styles: &[
        Style {
            prompt_path: "prompts/angry.txt",
            emoji: "🤬",
            premium: false,
        },
        // Style {
        //     prompt_path: "prompts/anime_geek.txt",
        //     emoji: "🇯🇵",
        //     premium: false,
        // },
        // Style {
        //     prompt_path: "prompts/churchly.txt",
        //     emoji: "🕯️",
        //     premium: false,
        // },
    ],
};

// Не знаю норм ли так оставлять но шо работает то работает
pub const MAIN_BUTTONS: StyleDict = StyleDict {
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

pub async fn start(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    let keyboard = make_keyboard(Language::get_vec(), 3);
    bot.send_message(msg.chat.id, "🗣️❓")
        .reply_markup(keyboard)
        .await?;
    dialogue.update(State::RecieveLanguage).await?;
    Ok(())
}

pub async fn receive_lang(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    if let Some(text) = msg.text() {
        if Language::is_supported(text) {
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

pub async fn receive_style(
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

pub async fn receive_location(
    bot: Bot,
    dialogue: MyDialogue,
    (language, style_path): (String, String),
    msg: Message,
    finder: Arc<DefaultFinder>,
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

        let user_settings = crate::bot::UserSettings {
            chat_id: msg.chat.id.0,
            language,
            style_path,
            timezone,
        };

        dialogue
            .update(State::WaitForTask { user_settings })
            .await?;
        log::info!("Updating to wait_for_task");
    } else {
        bot.send_message(msg.chat.id, "⌨️⌨️⌨️⌨️👇👇👇🤦🤦").await?;
    }

    Ok(())
}
