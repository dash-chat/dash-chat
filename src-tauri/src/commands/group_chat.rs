// #[command]
// pub async fn send_message(
//     chat_id: ChatId,
//     content: ChatMessageContent,
//     node: State<'_, Node>,
// ) -> Result<(), String> {
//     node.send_message(chat_id, content)
//         .await
//         .map_err(|e| format!("Failed to send message: {e:?}"))?;

//     Ok(())
// }

// #[command]
// pub async fn get_messages(
//     chat_id: ChatId,
//     node: State<'_, Node>,
// ) -> Result<Vec<ChatMessage>, String> {
//     node.get_messages(chat_id)
//         .await
//         .map_err(|e| format!("Failed to send message: {e:?}"))
// }
