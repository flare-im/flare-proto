//! MessageContent 扩展方法
//!
//! 提供统一的、高性能的 MessageContent 编码/解码方法
//! 使用 simd-json 优化 JSON 处理（如果需要）

use crate::common::MessageContent;
use prost::Message as ProstMessage;

/// MessageContent 扩展 trait
///
/// 提供统一的编码/解码接口，确保所有地方使用相同的实现
pub trait MessageContentExt {
    /// 编码 MessageContent 为字节数组
    ///
    /// # 性能
    /// - 使用 prost 的原生编码，性能最优
    /// - 零拷贝（如果可能）
    ///
    /// # 返回
    /// - `Ok(Vec<u8>)` - 编码后的字节数组
    /// - `Err(prost::EncodeError)` - 编码失败
    fn encode_to_bytes(&self) -> Result<Vec<u8>, prost::EncodeError>;

    /// 从字节数组解码 MessageContent
    ///
    /// # 性能
    /// - 使用 prost 的原生解码，性能最优
    /// - 借用反序列化（如果可能）
    ///
    /// # 参数
    /// - `bytes: &[u8]` - 编码后的字节数组
    ///
    /// # 返回
    /// - `Ok(MessageContent)` - 解码后的 MessageContent
    /// - `Err(prost::DecodeError)` - 解码失败
    fn decode_from_bytes(bytes: &[u8]) -> Result<Self, prost::DecodeError>
    where
        Self: Sized;
}

impl MessageContentExt for MessageContent {
    #[inline]
    fn encode_to_bytes(&self) -> Result<Vec<u8>, prost::EncodeError> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;
        Ok(buf)
    }

    #[inline]
    fn decode_from_bytes(bytes: &[u8]) -> Result<Self, prost::DecodeError> {
        MessageContent::decode(bytes)
    }
}

/// 从 Message 中编码 typed content 字节。
///
/// # 参数
/// - `message: &crate::common::Message` - 含 content 字段的 Message
///
/// # 返回
/// - `Ok(Vec<u8>)` - content 字节（空则返回空 Vec）
/// - `Err(prost::EncodeError)` - 编码失败
#[inline]
pub fn encode_message_content(
    message: &crate::common::Message,
) -> Result<Vec<u8>, prost::EncodeError> {
    match &message.content {
        Some(content) => content.encode_to_bytes(),
        None => Ok(Vec::new()),
    }
}

/// 从字节数组解码为 MessageContent
///
/// # 性能优化
/// - 使用 prost 的原生解码
/// - 零拷贝（如果可能）
///
/// # 参数
/// - `bytes: &[u8]` - 编码后的字节数组
///
/// # 返回
/// - `Ok(MessageContent)` - 解码成功
/// - `Err(prost::DecodeError)` - 解码失败
#[inline]
pub fn decode_message_content(bytes: &[u8]) -> Result<MessageContent, prost::DecodeError> {
    MessageContent::decode_from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{TextContent, message_content::Content};

    #[test]
    fn test_encode_decode() {
        let content = MessageContent {
            content: Some(Content::Text(TextContent {
                text: "Hello, World!".to_string(),
                mentions: Vec::new(),
            })),
        };

        let encoded = content.encode_to_bytes().unwrap();
        assert!(!encoded.is_empty());

        let decoded = MessageContent::decode_from_bytes(&encoded).unwrap();
        match decoded.content {
            Some(Content::Text(text_content)) => {
                assert_eq!(text_content.text, "Hello, World!");
            }
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_encode_message_content_none() {
        let message = crate::common::Message::default();
        let encoded = encode_message_content(&message).unwrap();
        assert!(encoded.is_empty());
    }
}
