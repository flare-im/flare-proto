# Flare IM Proto 架构设计（DDD + CQRS）

> 对标微信 / 飞书 / Telegram / Discord 的聊天基座；面向生产、可扩展、跨端一致。  
> 所有状态变更经 **Domain Event** 驱动；**Command** 写事件流，**Query** 读读模型。

---

## 1. Bounded Context 与包边界

| Bounded Context | 职责 | Proto 映射 | 聚合根 / 读模型 |
|-----------------|------|------------|------------------|
| **Connection** | 长连接生命周期、认证、心跳、设备绑定 | common/transport_data.proto (DATA 通道自定义载荷), online.proto | ConnectionId, DeviceSession |
| **Message** | 消息发送、会话内顺序、消息 FSM | common/message.proto, common/event.proto | Message (seq 为序)，Event 流 |
| **Session** | 会话列表、未读、游标、预览（读模型） | common/conversation.proto, common/sync.proto | ConversationLight, ConversationSummary |
| **Sync** | 按会话拉取事件、会话列表增量 | common/sync.proto, common/event.proto | EventEnvelope, ConversationPatch |
| **Presence** | 在线状态、输入状态 | common/event.proto (PresenceEvent, TypingEvent) | 无聚合根，事件即状态 |
| **Push** | 离线推送、回执 | common/ack.proto, push.proto | - |

- **tenant_id / operator_id / request_id**：统一由 gRPC metadata 注入（见 common/metadata.proto），请求体不重复携带。
- **Identity**：消息以 `(conversation_id, seq)` 为会话内全序；`server_id` 全局唯一。同步锚点为 `last_seq`。

---

## 2. CQRS 划分

### 2.1 Command（写侧）

- **SendMessage**：客户端上行一条消息草稿（Message，仅填可写字段）；服务端写事件流，分配 server_id/seq/timestamp，并产生 `Event(EVENT_MESSAGE, message)`。
- **ExecuteEvent**：所有“操作类”变更统一入口：撤回、编辑、删除、已读、反应、置顶、标记等，均编码为 `Event(type + payload)`，服务端写事件流并回 `OperationResponse(request_id + RpcStatus)`。
- 长连接：ClientPacket 中 `send_message = Message`、`send_event = Event`，与 gRPC 的 SendMessage / ExecuteEvent 语义一致。

### 2.2 Query（读侧）

- **Sync（按会话）**：SyncRequest(conversation_id, last_seq, limit) → SyncResponse(EventEnvelope)。客户端用 `last_seq` 拉取该会话后续事件，读模型由事件流消费生成。
- **会话列表**：SyncConversationsRequest / ConversationSyncAllRequest → patches 或 full list（ConversationSummary / ConversationLight）。
- **会话详情**：GetConversationDetail(conversation_id) → ConversationDetail。
- **消息列表**：QueryMessages(conversation_id, cursor, limit) → messages + next_cursor（读模型查询）。
- **单条消息 / 已读/编辑历史/反应**：GetMessage、GetMessageReadReceipts、GetMessageEditHistory、GetMessageReactions 等按需查询，不随 Message 主模型下发。

### 2.3 事件流与读模型

- 写侧：Command → 写 Event 到事件流（如 Kafka），并更新写模型（消息表、操作表等）。
- 读侧：消费事件流更新读模型（会话未读、预览、已读位点等）；Query 只读读模型。
- Event 类型与 message.proto 操作一一对应：EVENT_MESSAGE_RECALL → MessageRecallEvent，EVENT_READ_RECEIPT → ReadReceiptEvent，等。

---

## 3. 消息 FSM（显式状态机）

```
CREATED → SENT → DELIVERED → READ
                ↘ RECALLED | DELETED_SOFT | DELETED_HARD
```

- **Message.status** 即 FSM 状态；recalled_at / recall_reason 仅在 RECALLED 时有效。
- 编辑不改变主状态（仍 SENT），仅 current_edit_version / last_edited_at 变更；完整编辑历史走 GetMessageEditHistory 查询。

---

## 4. 连接与传输（Connection BC）

- **ClientPacket**：oneof { send_message(Message), send_event(Event), sync_request, sync_conversations, sync_conversations_all, get_conversation_detail, push_ack }。
- **ServerPacket**：oneof { event_envelope, send_ack, operation_response, sync_resp, sync_conversations_resp, ..., custom_push, error }。
- 认证 / 租户 / 操作者：连接建立时或首包从 metadata/Token 解析，不在每条 ClientPacket 重复。

---

## 5. 同步模型（Telegram/微信风格）

- **按会话同步**：客户端维护 per-conversation 的 `last_seq`；SyncRequest(conversation_id, last_seq, limit) 返回 EventEnvelope(events, max_seq, has_more, next_cursor)。
- **会话列表**：全量或增量（cursor）拉取 ConversationPatch；轻量用 ConversationLight，详情用 GetConversationDetail。
- **多端漫游**：同一用户多设备共享事件流；last_seq 按会话一致，未读/已读由 ReadReceiptEvent 与读模型维护。

---

## 6. Proto 文件清单与依赖

```
common/
  enums.proto       # DeleteType、ReactionAction、MarkType（共享枚举，避免 event↔message 循环依赖）
  metadata.proto    # 身份/请求约定、Trace、Actor、Device、Tenant、Pagination
  errors.proto      # ErrorCode、RpcStatus、RetryAdvice、ErrorDetail
  message.proto     # Message（聚合根）、MessageStatus（FSM）、MessageTimeline/MessageReadRecord（按需）
  message_content.proto  # MessageContent oneof、MessageType、各 Content 类型
  event.proto # 领域事件（含 Pin/Reaction/Mark/Edit 等事件 payload）
  models.proto # 查询读模型（PinnedMessageInfo、Reaction、EditHistory、ThreadInfo、MarkedMessageInfo，由事件流产生、按需返回）
  event.proto       # Event、EventType、EventEnvelope、所有 *Event payload（依赖 message、enums）
  sync.proto        # SyncRequest/Response、ConversationSync*、ConversationPatch
  conversation.proto # ConversationLight/Summary/Detail、MessagePreview（读模型）
  transport_data.proto  # DATA 通道自定义载荷（CustomPushData），不占会话 seq
  ack.proto         # SendAck、OperationResponse、PushAck、SendEnvelopeAck、ErrorPacket
```

**依赖顺序（无环）**：enums → message_content；message → message_content；models → enums, message_content；event → enums, message；sync → event, conversation；transport_data 仅定义 CustomPushData，无依赖。

- 服务层（message.proto RPC、conversation.proto、access_gateway.proto、push.proto、online.proto、router.proto、storage.proto、capability_service.proto（含 HookExtension / HookService）、media.proto）仅定义 RPC 与专属 Request/Response，身份与错误复用 common。

### 6.1 服务边界（与白皮书三条流对齐）

| 服务 | 职责 | 白皮书对应 |
|------|------|------------|
| **RouterService** (router.proto) | 实时信令中枢：SVID/分片路由、SelectPushTargets/GetDeviceRoute、流控；不访问 DB、不作事件总线 | §2 |
| **MessageService** (message.proto) | 写路径入口：SendMessage、ExecuteEvent(Event)；操作便捷 RPC；查询可委托 Storage Reader | §1.2 实时信令流、Orchestrator |
| **StorageReaderService** (storage.proto) | 只读查询与读模型；写仅由 Kafka Writer 消费，无 gRPC 写 | §3、§1.2 查询流 |
| **ConversationService** (conversation.proto) | 会话 CRUD、列表/增量同步、游标与未读、Thread；与 common/sync 语义一致 | §6 多端同步 |
| **OnlineService** (online.proto) | 连接生命周期（Login/Logout/Heartbeat）、在线状态与设备；Router 选端依赖 | §2.4 |
| **PushService** (push.proto) | 消息/通知推送、模板、调度、ACK；消费 push.tasks，调 Router 选端 | §4.1 |
| **AccessGateway** (access_gateway.proto) | 业务系统→客户端推送、连接查询、信令 Pub/Sub | Gateway 无状态代理 |
| **HookExtension / HookService**（`capability_service.proto`，包 `flare.capability.v1`） | 消息/推送/会话/在线生命周期 Hook 执行与配置 | §4.1 hook.requests |
| **CapabilityService**（同上 proto，包 `flare.capability.v1`） | 能力目录、用户授权、租户开关、能力命令分发（如 RTC） | SDK / 网关 → flare-capability |
| **MediaService** (media.proto) | 上传/下载、分片、引用、图片视频处理、ACL/桶 | 查询流可直连 |

---

## 7. Rust 侧落地要点

- **Domain**：Message 聚合根、Event 枚举与 payload、Conversation 读模型；Command/Query 严格分离。
- **Application**：SendMessage → 写 Event(EVENT_MESSAGE)；ExecuteEvent → 写对应 Event 并回 OperationResponse；SyncRequest → 从读模型或事件流返回 EventEnvelope。
- **Infrastructure**：gRPC/WebSocket/QUIC 从 metadata 取 tenant_id/actor_id/request_id；序列化使用 prost/tonic，与 proto 一一对应。
- **FSM**：Message 状态变更仅通过 Event（RECALL/EDIT/DELETE/READ 等）驱动，避免裸改 status 字段。

---

## 8. 与业界对齐

| 能力 | 微信/飞书/Telegram | 本设计 |
|------|---------------------|--------|
| 消息顺序 | 会话内递增 seq | conversation_id + seq，Sync 用 last_seq |
| 未读/已读 | 服务端维护，多端同步 | ReadReceiptEvent + 读模型，Query 按需 |
| 撤回/编辑 | 操作即事件 | Event(EVENT_MESSAGE_RECALL / EDIT) |
| 多端漫游 | 同一事件流 | 同一 last_seq 拉取 EventEnvelope |
| 离线推送 | 独立通道 | Push  BC，SendAck / PushAck |
| 扩展 | 自定义消息/事件 | MessageContent.custom，Event.custom |

本 proto 定义可直接支撑上述能力，无需为兼容旧协议保留冗余类型；实现时严格遵循「Command 写 Event、Query 读读模型」即可达到生产级一致性与可扩展性。

---

## 9. Rust 落地示例（Domain + Application 骨架）

### 9.1 聚合根与 FSM（Domain）

```rust
// domain/model/message.rs - Message 聚合根，状态仅经 Event 变更
#[derive(Debug, Clone)]
pub struct Message {
    pub server_id: String,
    pub conversation_id: String,
    pub client_msg_id: String,
    pub seq: u64,
    pub sender_id: String,
    pub content: MessageContent,
    pub status: MessageStatus,  // FSM
    pub recalled_at: Option<DateTime<Utc>>,
    pub current_edit_version: i32,
    pub last_edited_at: Option<DateTime<Utc>>,
    // ...
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageStatus { Created, Sent, Delivered, Read, Recalled, DeletedSoft, DeletedHard }

impl Message {
    pub fn can_transition_to(&self, next: MessageStatus) -> bool {
        matches!((self.status, next),
            (MessageStatus::Sent, MessageStatus::Recalled | MessageStatus::DeletedSoft | MessageStatus::DeletedHard)
            | (MessageStatus::Sent, MessageStatus::Delivered) | (MessageStatus::Delivered, MessageStatus::Read)
            | (MessageStatus::Created, MessageStatus::Sent))
    }
}
```

### 9.2 领域事件（Domain Event）

```rust
// domain/event.rs - 与 proto Event 一一对应
pub enum MessageEvent {
    MessageCreated { message: Message },
    MessageRecalled { server_msg_id: String, reason: String },
    MessageEdited { server_msg_id: String, new_content: MessageContent, edit_version: i32 },
    MessageDeleted { server_msg_id: String, delete_type: DeleteType },
    ReadReceipt { conversation_id: String, user_id: String, read_seq: u64, message_ids: Vec<String> },
    Reaction { server_msg_id: String, user_id: String, emoji: String, action: ReactionAction },
    Pin { server_msg_id: String, pinned_by: String, expire_at: Option<DateTime<Utc>> },
    Unpin { server_msg_id: String },
    Mark { server_msg_id: String, user_id: String, mark_type: MarkType },
    Unmark { server_msg_id: String, user_id: String, mark_type: Option<MarkType> },
}
```

### 9.3 Command / Query 端口（Application 层依赖反转）

```rust
// application/port.rs
pub trait MessageCommandPort: Send + Sync {
    async fn send_message(&self, ctx: &RequestContext, draft: MessageDraft) -> Result<SendResult>;
    async fn execute_event(&self, ctx: &RequestContext, event: Event) -> Result<OperationResponse>;
}

pub trait SyncQueryPort: Send + Sync {
    async fn sync_events(&self, conversation_id: &str, last_seq: u64, limit: i32) -> Result<EventEnvelope>;
    async fn list_conversations(&self, user_id: &str, cursor: Option<&str>, limit: i32) -> Result<Vec<ConversationPatch>>;
}

pub trait MessageQueryPort: Send + Sync {
    async fn query_messages(&self, conversation_id: &str, cursor: Option<&str>, limit: i32) -> Result<MessagePage>;
    async fn get_message(&self, message_id: &str) -> Result<Option<Message>>;
    async fn get_read_receipts(&self, message_id: &str) -> Result<Vec<MessageReadRecord>>;
    async fn get_edit_history(&self, message_id: &str) -> Result<Vec<EditHistory>>;
}
```

### 9.4 从 metadata 注入上下文（axum/tonic）

```rust
// infrastructure/grpc/metadata_ext.rs
pub fn extract_request_context(metadata: &MetadataMap) -> RequestContext {
    RequestContext {
        tenant_id: metadata.get("x-tenant-id").and_then(|v| v.to_str().ok()).map(String::from),
        actor_id: metadata.get("x-actor-id").and_then(|v| v.to_str().ok()).map(String::from),
        request_id: metadata.get("x-request-id").and_then(|v| v.to_str().ok()).map(String::from),
        ..Default::default()
    }
}
```

### 9.5 长连接分发（ClientPacket → Command）

```rust
// interface/ws/handler.rs
pub async fn handle_client_packet(packet: ClientPacket, ctx: RequestContext) -> Result<ServerPacket> {
    match packet.payload {
        Some(client_packet::Payload::SendMessage(m)) => {
            let result = message_command_port.send_message(&ctx, MessageDraft::from_proto(m)).await?;
            Ok(ServerPacket { payload: Some(server_packet::Payload::SendAck(result.into())) })
        }
        Some(client_packet::Payload::SendEvent(e)) => {
            let ack = message_command_port.execute_event(&ctx, Event::from_proto(e)).await?;
            Ok(ServerPacket { payload: Some(server_packet::Payload::OperationResponse(resp)) })
        }
        Some(client_packet::Payload::SyncRequest(req)) => {
            let envelope = sync_query_port.sync_events(&req.conversation_id, req.last_seq, req.limit).await?;
            Ok(ServerPacket { payload: Some(server_packet::Payload::SyncResp(SyncResponse { envelope: Some(envelope), status: None })) })
        }
        _ => { /* ... */ }
    }
}
```

以上与当前 proto 定义一一对应，可直接作为 message-orchestrator / gateway / storage-reader 的落地参考。
