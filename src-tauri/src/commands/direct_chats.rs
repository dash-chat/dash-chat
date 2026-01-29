use dashchat_node::{topic::kind::Chat, AgentId, ChatId, ChatMessageContent, ChatReaction, Node, Topic};
use tauri::State;

#[tauri::command]
pub fn direct_chat_id(peer: AgentId, node: State<'_, Node>) -> Topic<Chat> {
    Topic::direct_chat([node.agent_id(), peer])
}

#[tauri::command]
pub async fn direct_chat_send_message(
    chat_id: ChatId,
    content: ChatMessageContent,
    node: State<'_, Node>,
) -> Result<(), String> {
    node.send_message(chat_id, content)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(())
}


#[tauri::command]
pub async fn direct_chat_send_reaction(
    chat_id: ChatId,
    content: ChatReaction,
    node: State<'_, Node>,
) -> Result<(), String> {
    node.add_reaction(chat_id, content)
        .await
        .map_err(|err| format!("{err:?}"))?;
    Ok(())
}
