# 消息扩展指南（通用 IM 基座）

基座只定义与具体业务无关的通用类型，业务扩展通过 **MESSAGE_TYPE_SYSTEM + SystemContent** 或 **MESSAGE_TYPE_CUSTOM + CustomContent** 实现，无需修改基座枚举。

## 0. 内容格式约定

- **Message.content** 为强类型 `MessageContent`（见 message_content.proto）。`message_type` 与 oneof 变体保持一致（如 TEXT→TextContent，IMAGE→ImageContent）。
- **不再使用 ContentType 枚举**：文本/富媒体「格式」由 **MessageType** 唯一表达（如 TEXT=纯文本，MARKDOWN=Markdown，RICH_TEXT=富文本）。存储或展示时可根据 `message_type` 推断 MIME（如 1→text/plain，31→text/markdown）。

## 1. 系统事件（入群 / 退群 / 解散等）

使用 **MESSAGE_TYPE_SYSTEM**，具体事件由 `SystemContent.event_kind` 区分。

| event_kind 建议值           | 说明           |
|----------------------------|----------------|
| `group.member_joined`      | 某人加入群聊   |
| `group.member_left`        | 某人离开群聊   |
| `group.member_kicked`      | 某人被移出群   |
| `group.dissolved`          | 群已解散       |
| `group.created`            | 群创建成功     |
| `group.name_changed`       | 群名称已修改   |
| `group.avatar_changed`     | 群头像已修改   |
| `group.admin_granted`      | 被设为管理员   |
| `group.admin_revoked`      | 管理员身份撤销 |
| `channel.announcement`     | 频道公告       |

示例（入群）：

- `message_type` = `MESSAGE_TYPE_SYSTEM`
- `content.system.event_kind` = `"group.member_joined"`
- `content.system.body` = `"张三加入了群聊"`
- `content.system.attributes` = `{"user_id": "u1", "operator_id": "u2"}`

## 2. 业务自定义（红包 / 转账 / 打卡等）

使用 **MESSAGE_TYPE_CUSTOM**，具体业务由 `CustomContent.type` 区分，载荷放在 `CustomContent.payload`（自解析 JSON 或二进制）。

| type 建议值   | 说明     |
|---------------|----------|
| `red_packet`  | 红包     |
| `transfer`    | 转账     |
| `check_in`    | 打卡     |
| `vote`        | 投票     |
| `approval`    | 审批     |

示例（红包）：

- `message_type` = `MESSAGE_TYPE_CUSTOM`
- `content.custom.type` = `"red_packet"`
- `content.custom.description` = `"红包"`（会话列表展示）
- `content.custom.payload` = 业务序列化（如 JSON：`{"amount":100,"count":5,"greeting":"恭喜发财"}`）

## 3. Message.extensions

与消息一起透传、基座不解析的数据可放入 `Message.extensions`，key 建议带命名空间，例如：

- `biz.red_packet.extra`
- `biz.transfer.sn`

基座只做透传，存储与同步不变。

## 4. 枚举扩展（可选）

若业务希望使用独立枚举值而非 CUSTOM + type 字符串，可在 **116–199** 区间增加自定 `MessageType`（需改基座 proto 或通过 import 扩展）。更推荐用 **MESSAGE_TYPE_CUSTOM + type 字符串**，避免基座枚举膨胀。
