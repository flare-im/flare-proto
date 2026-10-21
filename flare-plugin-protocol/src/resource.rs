//! Host capabilities are explicit, read-only, and scoped to the conversation
//! bound by the invoking user. Raw SQL, arbitrary URLs and bearer forwarding
//! are deliberately absent from this contract.
use crate::{ProtocolError, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_RESOURCE_BYTES: usize = 64 * 1024;
pub const MAX_RESOURCE_CALLS: usize = 8;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum HostCapability {
    #[serde(rename = "im.conversation.read")]
    ConversationRead,
    #[serde(rename = "im.conversation.participants.read")]
    ConversationParticipantsRead,
    #[serde(rename = "social.group.member.read")]
    SocialGroupMemberRead,
}
impl HostCapability {
    pub fn id(self) -> &'static str {
        match self {
            Self::ConversationRead => "im.conversation.read",
            Self::ConversationParticipantsRead => "im.conversation.participants.read",
            Self::SocialGroupMemberRead => "social.group.member.read",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "capability", deny_unknown_fields)]
pub enum ResourceRequest {
    #[serde(rename = "im.conversation.read")]
    ConversationRead {},
    #[serde(rename = "im.conversation.participants.read")]
    ConversationParticipantsRead { cursor: String, limit: u32 },
    #[serde(rename = "social.group.member.read")]
    SocialGroupMemberRead {
        #[serde(rename = "userId")]
        user_id: String,
    },
}
impl ResourceRequest {
    pub fn capability(&self) -> HostCapability {
        match self {
            Self::ConversationRead {} => HostCapability::ConversationRead,
            Self::ConversationParticipantsRead { .. } => {
                HostCapability::ConversationParticipantsRead
            }
            Self::SocialGroupMemberRead { .. } => HostCapability::SocialGroupMemberRead,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ConversationRead {} => Ok(()),
            Self::ConversationParticipantsRead { cursor, limit }
                if (1..=100).contains(limit)
                    && cursor.len() <= 128
                    && cursor.bytes().all(|b| b.is_ascii_digit()) =>
            {
                Ok(())
            }
            Self::SocialGroupMemberRead { user_id } if valid_resource_id(user_id) => Ok(()),
            _ => Err(ProtocolError::Denied("invalid resource request")),
        }
    }
}

pub fn valid_resource_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':'))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationView {
    pub conversation_id: String,
    pub conversation_type: String,
    pub display_name: String,
    pub member_count: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticipantView {
    pub user_id: String,
    pub roles: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParticipantPage {
    pub conversation_id: String,
    pub participants: Vec<ParticipantView>,
    pub next_cursor: String,
    pub has_more: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupMemberView {
    pub group_id: String,
    pub user_id: String,
    pub roles: Vec<i32>,
    pub muted: bool,
}

/// Internal trusted Core-to-Social request. A plugin never chooses group_id:
/// Core resolves it from the current, member-authorized conversation detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupMemberLookup {
    pub group_id: String,
    pub user_id: String,
}
