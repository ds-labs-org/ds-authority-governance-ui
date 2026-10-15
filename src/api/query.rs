//! EDC `QuerySpec` bodies for the hub's `POST .../query` list endpoints.

use serde::Serialize;
use serde_json::Value;

/// Hub-side cap on `limit`.
pub const MAX_LIMIT: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[allow(dead_code)] // `Asc` is API surface; the views sort newest first
pub enum SortOrder {
    #[serde(rename = "ASC")]
    Asc,
    #[serde(rename = "DESC")]
    Desc,
}

/// One `filterExpression` entry. The hub supports only `=`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Criterion {
    pub operand_left: String,
    pub operator: String,
    pub operand_right: Value,
}

/// A QuerySpec. Every field is optional on the wire; unset fields are
/// omitted, so `QuerySpec::default()` serializes to `{}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filter_expression: Vec<Criterion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<SortOrder>,
}

impl QuerySpec {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything the hub will return in one page (`limit` = 500), newest
    /// first by `sort_field`.
    pub fn all_newest_first(sort_field: &str) -> Self {
        Self::new()
            .limit(MAX_LIMIT)
            .sort(sort_field, SortOrder::Desc)
    }

    #[allow(dead_code)] // API surface (paging); the views load one capped page
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Sets `limit`, clamped to [`MAX_LIMIT`].
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit.min(MAX_LIMIT));
        self
    }

    /// Adds `field = value`.
    pub fn filter(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.filter_expression.push(Criterion {
            operand_left: field.to_string(),
            operator: "=".to_string(),
            operand_right: value.into(),
        });
        self
    }

    pub fn sort(mut self, field: &str, order: SortOrder) -> Self {
        self.sort_field = Some(field.to_string());
        self.sort_order = Some(order);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_spec_serializes_to_empty_object() {
        assert_eq!(serde_json::to_value(QuerySpec::new()).unwrap(), json!({}));
    }

    #[test]
    fn full_spec_matches_contract_example() {
        let spec = QuerySpec::new()
            .offset(0)
            .limit(50)
            .filter("status", "pending")
            .sort("createdAt", SortOrder::Desc);
        assert_eq!(
            serde_json::to_value(spec).unwrap(),
            json!({
                "offset": 0,
                "limit": 50,
                "filterExpression": [
                    {"operandLeft": "status", "operator": "=", "operandRight": "pending"}
                ],
                "sortField": "createdAt",
                "sortOrder": "DESC"
            })
        );
    }

    #[test]
    fn filters_accumulate_in_order() {
        let spec = QuerySpec::new().filter("a", 1).filter("b", true);
        assert_eq!(
            serde_json::to_value(spec).unwrap()["filterExpression"],
            json!([
                {"operandLeft": "a", "operator": "=", "operandRight": 1},
                {"operandLeft": "b", "operator": "=", "operandRight": true}
            ])
        );
    }

    #[test]
    fn limit_is_clamped_to_hub_cap() {
        assert_eq!(QuerySpec::new().limit(10_000).limit, Some(MAX_LIMIT));
    }

    #[test]
    fn all_newest_first_uses_max_limit_and_desc_sort() {
        assert_eq!(
            serde_json::to_value(QuerySpec::all_newest_first("issuedAt")).unwrap(),
            json!({"limit": 500, "sortField": "issuedAt", "sortOrder": "DESC"})
        );
    }
}
