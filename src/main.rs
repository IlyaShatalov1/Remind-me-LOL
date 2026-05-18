mod bot;
mod get_user_info;
mod reminders;
mod utils;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    bot::run().await;
}
