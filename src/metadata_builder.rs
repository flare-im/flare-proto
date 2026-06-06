//! 元数据/上下文类型的便捷构建
//!
//! 与 `proto/common/metadata.proto` 对齐，提供 `Pagination`、`FilterExpression`、
//! `SortExpression`、`ActorContext`、`DeviceContext`、`TimeRange`、`AuditContext` 等
//! 工厂函数与便捷方法，便于业务层快速组装查询条件与上下文。

use std::collections::HashMap;

use crate::common::{
    ActorContext, ActorType, AuditContext, DeviceContext, DevicePriority, FilterExpression,
    FilterOperator, Pagination, SortDirection, SortExpression, TimeRange,
};

// ----------------------------------------------------------------------------
// ActorContext
// ----------------------------------------------------------------------------

/// 用户操作者
#[inline]
pub fn actor_user(actor_id: impl Into<String>) -> ActorContext {
    ActorContext {
        actor_id: actor_id.into(),
        r#type: ActorType::User as i32,
        roles: vec![],
        attributes: HashMap::new(),
    }
}

/// 服务操作者
#[inline]
pub fn actor_service(actor_id: impl Into<String>) -> ActorContext {
    ActorContext {
        actor_id: actor_id.into(),
        r#type: ActorType::Service as i32,
        roles: vec![],
        attributes: HashMap::new(),
    }
}

/// 系统操作者
#[inline]
pub fn actor_system(actor_id: impl Into<String>) -> ActorContext {
    ActorContext {
        actor_id: actor_id.into(),
        r#type: ActorType::System as i32,
        roles: vec![],
        attributes: HashMap::new(),
    }
}

/// 租户管理员操作者
#[inline]
pub fn actor_tenant_admin(actor_id: impl Into<String>) -> ActorContext {
    ActorContext {
        actor_id: actor_id.into(),
        r#type: ActorType::TenantAdmin as i32,
        roles: vec![],
        attributes: HashMap::new(),
    }
}

/// 为 ActorContext 添加 roles（返回新实例，便于链式调用）
#[inline]
pub fn actor_with_roles(
    mut actor: ActorContext,
    roles: impl IntoIterator<Item = impl Into<String>>,
) -> ActorContext {
    actor.roles = roles.into_iter().map(Into::into).collect();
    actor
}

/// 为 ActorContext 添加 attributes
#[inline]
pub fn actor_with_attributes(
    mut actor: ActorContext,
    attrs: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
) -> ActorContext {
    actor.attributes = attrs
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
    actor
}

// ----------------------------------------------------------------------------
// DeviceContext
// ----------------------------------------------------------------------------

/// 创建设备上下文（与 metadata.proto DeviceContext 一致：device_id、platform、priority）
#[inline]
pub fn device_context(device_id: impl Into<String>, platform: impl Into<String>) -> DeviceContext {
    DeviceContext {
        device_id: device_id.into(),
        platform: platform.into(),
        priority: DevicePriority::Unspecified as i32,
    }
}

/// 设置设备优先级（当前使用设备）
#[inline]
pub fn device_with_priority_high(mut device: DeviceContext) -> DeviceContext {
    device.priority = DevicePriority::High as i32;
    device
}

/// 设置设备优先级（后台/离线）
#[inline]
pub fn device_with_priority_low(mut device: DeviceContext) -> DeviceContext {
    device.priority = DevicePriority::Low as i32;
    device
}

/// 设置设备优先级（强制推送）
#[inline]
pub fn device_with_priority_critical(mut device: DeviceContext) -> DeviceContext {
    device.priority = DevicePriority::Critical as i32;
    device
}

// ----------------------------------------------------------------------------
// Pagination
// ----------------------------------------------------------------------------

/// 分页（cursor + limit；has_more/total_size 由服务端填充）
#[inline]
pub fn pagination(cursor: impl Into<String>, limit: i32) -> Pagination {
    Pagination {
        cursor: cursor.into(),
        limit,
        has_more: false,
        previous_cursor: String::new(),
        total_size: 0,
    }
}

/// 首页分页（空 cursor）
#[inline]
pub fn pagination_first(limit: i32) -> Pagination {
    pagination("", limit)
}

/// 分页并标记是否有更多
#[inline]
pub fn pagination_with_more(
    cursor: impl Into<String>,
    limit: i32,
    has_more: bool,
    total_size: i64,
) -> Pagination {
    Pagination {
        cursor: cursor.into(),
        limit,
        has_more,
        previous_cursor: String::new(),
        total_size,
    }
}

// ----------------------------------------------------------------------------
// SortExpression
// ----------------------------------------------------------------------------

/// 升序排序
#[inline]
pub fn sort_asc(field: impl Into<String>) -> SortExpression {
    SortExpression {
        field: field.into(),
        direction: SortDirection::Asc as i32,
    }
}

/// 降序排序
#[inline]
pub fn sort_desc(field: impl Into<String>) -> SortExpression {
    SortExpression {
        field: field.into(),
        direction: SortDirection::Desc as i32,
    }
}

// ----------------------------------------------------------------------------
// FilterExpression
// ----------------------------------------------------------------------------

/// 等于
#[inline]
pub fn filter_eq(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Eq as i32,
        values: vec![value.into()],
    }
}

/// 不等于
#[inline]
pub fn filter_ne(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Ne as i32,
        values: vec![value.into()],
    }
}

/// IN
#[inline]
pub fn filter_in(
    field: impl Into<String>,
    values: impl IntoIterator<Item = impl Into<String>>,
) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::In as i32,
        values: values.into_iter().map(Into::into).collect(),
    }
}

/// NOT IN
#[inline]
pub fn filter_not_in(
    field: impl Into<String>,
    values: impl IntoIterator<Item = impl Into<String>>,
) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::NotIn as i32,
        values: values.into_iter().map(Into::into).collect(),
    }
}

/// 大于
#[inline]
pub fn filter_gt(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Gt as i32,
        values: vec![value.into()],
    }
}

/// 大于等于
#[inline]
pub fn filter_ge(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Ge as i32,
        values: vec![value.into()],
    }
}

/// 小于
#[inline]
pub fn filter_lt(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Lt as i32,
        values: vec![value.into()],
    }
}

/// 小于等于
#[inline]
pub fn filter_le(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Le as i32,
        values: vec![value.into()],
    }
}

/// 包含（字符串）
#[inline]
pub fn filter_contains(field: impl Into<String>, value: impl Into<String>) -> FilterExpression {
    FilterExpression {
        field: field.into(),
        op: FilterOperator::Contains as i32,
        values: vec![value.into()],
    }
}

// ----------------------------------------------------------------------------
// TimeRange
// ----------------------------------------------------------------------------

/// 时间范围。所有时间点均为 Unix epoch milliseconds。
#[inline]
pub fn time_range(start: Option<i64>, end: Option<i64>) -> TimeRange {
    TimeRange {
        start_time: start,
        end_time: end,
    }
}

/// 从 Unix 秒转换为协议内使用的毫秒时间点。
#[inline]
pub fn unix_millis_from_seconds(secs: i64) -> i64 {
    secs.saturating_mul(1000)
}

// ----------------------------------------------------------------------------
// AuditContext
// ----------------------------------------------------------------------------

/// 审计上下文
#[inline]
pub fn audit_context(
    actor: ActorContext,
    operated_at: Option<i64>,
    reason: impl Into<String>,
) -> AuditContext {
    AuditContext {
        actor: Some(actor),
        operated_at,
        reason: reason.into(),
        attributes: HashMap::new(),
    }
}
