//! lemurclaw 传输抽象:让 GUI(wry 进程内)和 WebUI(WebSocket)共用同一套协议逻辑。
//! 此 crate 独立,不含 wry/tao——webui 模式只依赖它,不背 GUI 重依赖。

use codex_app_server_protocol::{ClientRequest, ServerNotification, ServerRequest};

/// 从后端到达前端的事件(包装 codex 的三类消息)。
#[derive(Debug, Clone)]
pub enum ServerEvent {
    Notification(ServerNotification),
    Request(ServerRequest),
}

/// 传输抽象。WryIpc(lemurclaw-gui)和 WebSocket(webui)各一实现。
pub trait Transport: Send {
    async fn send(&self, req: ClientRequest) -> std::io::Result<()>;
    async fn recv(&mut self) -> std::io::Result<Option<ServerEvent>>;
}

/// JSON 编码任意协议消息(codex JSON-RPC 格式)。
pub fn encode<T: serde::Serialize>(msg: &T) -> std::io::Result<String> {
    serde_json::to_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// JSON 解码任意协议消息。
pub fn decode<'de, T: serde::Deserialize<'de>>(json: &'de str) -> std::io::Result<T> {
    serde_json::from_str(json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ClientNotification;

    #[test]
    fn message_round_trip() {
        let msg = ClientNotification::Initialized;
        let json = encode(&msg).expect("encode");
        let back: ClientNotification = decode(&json).expect("decode");
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}
