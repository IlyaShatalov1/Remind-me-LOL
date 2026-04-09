mod bot;

#[tokio::main]
async fn main() {
    bot::run().await;
}

// /// Parse the text wrote on Telegram and check if that text is a valid command
// /// or not, then match the command. If the command is `/start` it writes a
// /// markup with the `InlineKeyboardMarkup`.
// async fn message_handler(
//     bot: Bot,
//     msg: Message,
//     me: Me,
// ) -> Result<(), Box<dyn Error + Send + Sync>> {
//     if let Some(text) = msg.text() {
//         match BotCommands::parse(text, me.username()) {
//             Ok(Command::Help) => {
//                 // Just send the description of all commands.
//                 bot.send_message(msg.chat.id, Command::descriptions().to_string())
//                     .await?;
//             }
//             Ok(Command::Start) => {
//                 // Create a list of buttons and send them.
//                 // Создание клавиатуры пример
//                 let keyboard =
//                     KeyboardMarkup::new(vec![vec![KeyboardButton::new("alo")]]).resize_keyboard();
//                 bot.send_message(msg.chat.id, "Debian versions:")
//                     .reply_markup(keyboard)
//                     .await?;
//             }

//             Err(_) => {
//                 bot.send_message(msg.chat.id, "Command not found!").await?;
//             }
//         }
//     }

//     Ok(())
// }

// async fn inline_query_handler(
//     bot: Bot,
//     q: InlineQuery,
// ) -> Result<(), Box<dyn Error + Send + Sync>> {
//     let choose_debian_version = InlineQueryResultArticle::new(
//         "0",
//         "Chose debian version",
//         InputMessageContent::Text(InputMessageContentText::new("Debian versions:")),
//     );
//     //.reply_markup(make_inline_keyboardkeyboard());

//     bot.answer_inline_query(q.id, vec![choose_debian_version.into()])
//         .await?;

//     Ok(())
// }

// /// When it receives a callback from a button it edits the message with all
// /// those buttons writing a text with the selected Debian version.
// ///
// /// **IMPORTANT**: do not send privacy-sensitive data this way!!!
// /// Anyone can read data stored in the callback button.
// async fn callback_handler(bot: Bot, q: CallbackQuery) -> Result<(), Box<dyn Error + Send + Sync>> {
//     if let Some(version) = q.data.as_ref() {
//         let text = format!("You chose: {version}");

//         // Tell telegram that we've seen this query, to remove 🕑 icons from the
//         // clients. You could also use `answer_callback_query`'s optional
//         // parameters to tweak what happens on the client side.
//         bot.answer_callback_query(q.id.clone()).await?;

//         // Edit text of the message to which the buttons were attached
//         if let Some(message) = q.regular_message() {
//             bot.edit_text(message, text).await?;
//         } else if let Some(id) = q.inline_message_id {
//             bot.edit_message_text_inline(id, text).await?;
//         }

//         log::info!("You chose: {version}");
//     }

//     Ok(())
// }
