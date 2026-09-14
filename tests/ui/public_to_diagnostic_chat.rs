use teloxide::{Bot, types::ChatId};
use untisgram::{
    message::{Text, render_public},
    utils::send_or_edit_message,
    work::Chat,
};

async fn send(bot: Bot) {
    let chat = Chat::diagnostic(ChatId(1), None);
    let rendered = render_public(Text("public"));
    let _ = send_or_edit_message(&bot, chat, rendered, &mut None).await;
}

fn main() {
    let _ = send(Bot::new("token"));
}
