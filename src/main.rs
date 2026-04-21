mod bot;
mod get_user_info;
mod reminders;
mod utils;

#[tokio::main]
async fn main() {
    bot::run().await;
}
