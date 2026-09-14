use teloxide::{Bot, types::ChatId};
use untisgram::{
    message::{Text, render_both},
    utils::send_or_edit_message,
    work::Chat,
};

async fn send(bot: Bot) {
    let chat = Chat::public(ChatId(1), None);
    let rendered = render_both(Text("secret"));
    let _ = send_or_edit_message(&bot, chat, rendered.diagnostic, &mut None).await;
}

fn main() {
    let _ = send(Bot::new("token"));
}
