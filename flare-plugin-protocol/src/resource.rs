//! Host capabilities are explicit. Reads ([`ResourceRequest`]) are scoped to the
//! conversation bound by the invoking user. Writes ([`ResourceWriteRequest`])
//! publish shared state for an entity of the calling plugin, deliver a
//! non-durable realtime packet to members of one conversation, or dispatch a
//! delegated service operation to another plugin. Raw SQL, arbitrary URLs and
//! bearer forwarding are deliberately absent from this contract.
use crate::{ProtocolError, Result, state::SharedState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A resource response body. A participants page of 500 with 128-byte user ids
/// stays under it.
pub const MAX_RESOURCE_BYTES: usize = 128 * 1024;
pub const MAX_RESOURCE_CALLS: usize = 8;
/// Participants per page of `im.conversation.participants.read` (`limit`).
pub const MAX_PARTICIPANTS_PAGE: u32 = 500;
/// User ids per `im.conversation.members.check` request.
pub const MAX_MEMBERS_CHECK_USER_IDS: usize = 50;
/// A resource request body: an [`ResourceWriteRequest::EntityStatePublish`]
/// carries up to [`crate::state::MAX_SHARED_STATE_BYTES`] of values.
pub const MAX_RESOURCE_REQUEST_BYTES: usize = 12 * 1024;
/// Recipients per [`RealtimeDelivery`]; 32 ids of 128 bytes plus a full
/// payload stay under [`MAX_RESOURCE_REQUEST_BYTES`].
pub const MAX_REALTIME_RECIPIENTS: usize = 32;
/// Serialized `payload` of a [`RealtimeDelivery`].
pub const MAX_REALTIME_PAYLOAD_BYTES: usize = 6 * 1024;
pub const MAX_REALTIME_PACKET_TYPE_BYTES: usize = 64;
/// A realtime packet is dropped by clients once this old.
pub const MAX_REALTIME_TTL_MS: u32 = 120_000;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum HostCapability {
    #[serde(rename = "im.conversation.read")]
    ConversationRead,
    #[serde(rename = "im.conversation.participants.read")]
    ConversationParticipantsRead,
    /// Read: which of up to 50 user ids are members of the bound conversation
    /// ([`ResourceRequest::ConversationMembersCheck`], [`MembersCheckView`]).
    #[serde(rename = "im.conversation.members.check")]
    ConversationMembersCheck,
    #[serde(rename = "social.group.member.read")]
    SocialGroupMemberRead,
    /// Write: publish [`SharedState`] for an entity bound to a message of the
    /// calling plugin. Requested with [`ResourceWriteRequest`].
    #[serde(rename = "im.plugin.entity_state.publish")]
    EntityStatePublish,
    /// Write: deliver a non-durable packet to online devices of members of one
    /// conversation. Requested with [`ResourceWriteRequest::RealtimeDeliver`].
    #[serde(rename = "im.realtime.deliver")]
    RealtimeDeliver,
    #[serde(rename = "plugin.service.dispatch")]
    ServiceDispatch,
}
impl HostCapability {
    pub fn id(self) -> &'static str {
        match self {
            Self::ConversationRead => "im.conversation.read",
            Self::ConversationParticipantsRead => "im.conversation.participants.read",
            Self::ConversationMembersCheck => "im.conversation.members.check",
            Self::SocialGroupMemberRead => "social.group.member.read",
            Self::EntityStatePublish => "im.plugin.entity_state.publish",
            Self::RealtimeDeliver => "im.realtime.deliver",
            Self::ServiceDispatch => "plugin.service.dispatch",
        }
    }
    /// Writes take a [`ResourceWriteRequest`]; every other capability is a read.
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Self::EntityStatePublish | Self::RealtimeDeliver | Self::ServiceDispatch
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "capability", deny_unknown_fields)]
pub enum ResourceRequest {
    #[serde(rename = "im.conversation.read")]
    ConversationRead {},
    /// `limit` is `1..=500`; answered with a [`ParticipantPage`].
    #[serde(rename = "im.conversation.participants.read")]
    ConversationParticipantsRead { cursor: String, limit: u32 },
    /// `{"capability":"im.conversation.members.check","userIds":[...]}`:
    /// `1..=50` unique user ids, answered with a [`MembersCheckView`] for the
    /// conversation bound by the ticket.
    #[serde(rename = "im.conversation.members.check")]
    ConversationMembersCheck {
        #[serde(rename = "userIds")]
        user_ids: Vec<String>,
    },
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
            Self::ConversationMembersCheck { .. } => HostCapability::ConversationMembersCheck,
            Self::SocialGroupMemberRead { .. } => HostCapability::SocialGroupMemberRead,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ConversationRead {} => Ok(()),
            Self::ConversationParticipantsRead { cursor, limit }
                if (1..=MAX_PARTICIPANTS_PAGE).contains(limit)
                    && cursor.len() <= 128
                    && cursor.bytes().all(|b| b.is_ascii_digit()) =>
            {
                Ok(())
            }
            Self::ConversationMembersCheck { user_ids }
                if (1..=MAX_MEMBERS_CHECK_USER_IDS).contains(&user_ids.len())
                    && user_ids.iter().all(|id| valid_resource_id(id))
                    && user_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == user_ids.len() =>
            {
                Ok(())
            }
            Self::SocialGroupMemberRead { user_id } if valid_resource_id(user_id) => Ok(()),
            _ => Err(ProtocolError::Denied("invalid resource request")),
        }
    }
}

/// The host write request, tagged by `capability` like [`ResourceRequest`]:
/// `{"capability": "im.plugin.entity_state.publish", "entity", "version", "values"}`.
///
/// It is authorized by the delegation ticket of a user or staff call whose
/// operation declares the capability, or by the plugin runtime credential
/// ([`crate::invocation::PluginRuntimeClaims`]) when no call is in flight. Core
/// writes only an entity bound to a message of the same tenant and plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", deny_unknown_fields)]
pub enum ResourceWriteRequest {
    #[serde(rename = "im.plugin.entity_state.publish")]
    EntityStatePublish(SharedState),
    #[serde(rename = "im.realtime.deliver")]
    RealtimeDeliver(RealtimeDelivery),
    #[serde(rename = "plugin.service.dispatch")]
    ServiceDispatch(crate::service::PluginServiceRequest),
}
impl ResourceWriteRequest {
    pub fn capability(&self) -> HostCapability {
        match self {
            Self::EntityStatePublish(_) => HostCapability::EntityStatePublish,
            Self::RealtimeDeliver(_) => HostCapability::RealtimeDeliver,
            Self::ServiceDispatch(_) => HostCapability::ServiceDispatch,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::EntityStatePublish(state) => state.validate(),
            Self::RealtimeDeliver(delivery) => delivery.validate(),
            Self::ServiceDispatch(request) => request.validate(),
        }
    }
}

/// `{"capability":"im.realtime.deliver","conversationId","userIds","packetType","payload","ttlMs"}`.
///
/// Core delivers the packet to the online devices of the listed users that
/// are members of the conversation: at most once, not stored, never pushed
/// offline. A delivery made under a user call ticket must name the
/// conversation the ticket is bound to; the runtime credential may name any
/// conversation of its tenant. Durable truth stays with the plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RealtimeDelivery {
    pub conversation_id: String,
    /// `1..=32` unique user ids.
    pub user_ids: Vec<String>,
    /// Lowercase `[a-z][a-z0-9._-]*`, at most 64 bytes, e.g. `call.snapshot`.
    pub packet_type: String,
    /// A JSON object of at most [`MAX_REALTIME_PAYLOAD_BYTES`] serialized.
    pub payload: serde_json::Map<String, serde_json::Value>,
    /// `1..=120000`: clients drop the packet once it is older than this.
    pub ttl_ms: u32,
}
impl RealtimeDelivery {
    pub fn validate(&self) -> Result<()> {
        let unique = self
            .user_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == self.user_ids.len();
        let payload_ok = serde_json::to_vec(&self.payload)
            .is_ok_and(|bytes| bytes.len() <= MAX_REALTIME_PAYLOAD_BYTES);
        if valid_resource_id(&self.conversation_id)
            && (1..=MAX_REALTIME_RECIPIENTS).contains(&self.user_ids.len())
            && self.user_ids.iter().all(|id| valid_resource_id(id))
            && unique
            && valid_packet_type(&self.packet_type)
            && payload_ok
            && (1..=MAX_REALTIME_TTL_MS).contains(&self.ttl_ms)
        {
            Ok(())
        } else {
            Err(ProtocolError::Denied("invalid realtime delivery"))
        }
    }
}

/// The answer to [`ResourceWriteRequest::RealtimeDeliver`]. `skipped` lists the
/// requested users that are not members of the conversation, in request order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RealtimeDeliveryView {
    pub delivery_id: String,
    pub delivered: u32,
    pub skipped: Vec<String>,
}

fn valid_packet_type(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_REALTIME_PACKET_TYPE_BYTES
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// What Core did with a publish; every outcome is a success for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EntityStatePublishOutcome {
    /// Newer than the stored version: stored and fanned out.
    Published,
    /// The stored version: nothing changed.
    Duplicate,
    /// Older than the stored version: dropped.
    Stale,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntityStatePublishView {
    pub entity: String,
    pub version: u64,
    pub outcome: EntityStatePublishOutcome,
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
/// The answer to [`ResourceRequest::ConversationMembersCheck`]:
/// `{"members":[...],"nonMembers":[...]}`. Every requested id is in exactly
/// one list, in request order; nothing else is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MembersCheckView {
    pub members: Vec<String>,
    pub non_members: Vec<String>,
}
impl MembersCheckView {
    /// Split `requested` (a validated request's `userIds`) by `is_member`,
    /// keeping request order: how a host builds the answer.
    pub fn partition(requested: &[String], mut is_member: impl FnMut(&str) -> bool) -> Self {
        let (members, non_members) = requested.iter().cloned().partition(|id| is_member(id));
        Self {
            members,
            non_members,
        }
    }
    /// Check an answer against the request's `userIds`: both lists together
    /// hold each requested id exactly once, in request order, and nothing else.
    pub fn validate_for(&self, requested: &[String]) -> Result<()> {
        let mut members = self.members.iter().peekable();
        let mut non_members = self.non_members.iter().peekable();
        let consistent = requested.iter().all(|id| {
            if members.peek() == Some(&id) {
                members.next();
                true
            } else if non_members.peek() == Some(&id) {
                non_members.next();
                true
            } else {
                false
            }
        });
        if consistent && members.next().is_none() && non_members.next().is_none() {
            Ok(())
        } else {
            Err(ProtocolError::Denied("invalid members check answer"))
        }
    }
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
