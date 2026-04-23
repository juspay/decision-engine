use clickhouse::{query::Query, Client};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindArg {
    String(String),
    I64(i64),
    U64(u64),
    Bool(bool),
}

impl BindArg {
    fn apply(self, query: Query) -> Query {
        match self {
            Self::String(value) => query.bind(value),
            Self::I64(value) => query.bind(value),
            Self::U64(value) => query.bind(value),
            Self::Bool(value) => query.bind(value),
        }
    }
}

impl From<String> for BindArg {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for BindArg {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<i64> for BindArg {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for BindArg {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl From<usize> for BindArg {
    fn from(value: usize) -> Self {
        Self::U64(value as u64)
    }
}

impl From<bool> for BindArg {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SqlFragment {
    sql: String,
    binds: Vec<BindArg>,
}

impl SqlFragment {
    pub fn new(sql: impl Into<String>) -> Self {
        Self {
            sql: sql.into(),
            binds: Vec::new(),
        }
    }

    pub fn with_binds(sql: impl Into<String>, binds: Vec<BindArg>) -> Self {
        Self {
            sql: sql.into(),
            binds,
        }
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn binds(&self) -> &[BindArg] {
        &self.binds
    }

    fn into_query(self, client: &Client) -> Query {
        let mut query = client.query(&self.sql);
        for bind in self.binds {
            query = bind.apply(query);
        }
        query
    }
}

#[derive(Debug, Clone)]
pub struct FilterClause {
    predicate: String,
    binds: Vec<BindArg>,
}

impl FilterClause {
    pub fn raw(predicate: impl Into<String>) -> Self {
        Self {
            predicate: predicate.into(),
            binds: Vec::new(),
        }
    }

    pub fn new(predicate: impl Into<String>, binds: Vec<BindArg>) -> Self {
        Self {
            predicate: predicate.into(),
            binds,
        }
    }

    pub fn eq(field: &'static str, value: impl Into<BindArg>) -> Self {
        Self::new(format!("{field} = ?"), vec![value.into()])
    }

    pub fn gte(field: &'static str, value: impl Into<BindArg>) -> Self {
        Self::new(format!("{field} >= ?"), vec![value.into()])
    }

    pub fn lte(field: &'static str, value: impl Into<BindArg>) -> Self {
        Self::new(format!("{field} <= ?"), vec![value.into()])
    }

    pub fn in_list(field: &'static str, values: &[String]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }

        let placeholders = std::iter::repeat_n("?", values.len())
            .collect::<Vec<_>>()
            .join(", ");
        Some(Self::new(
            format!("{field} IN ({placeholders})"),
            values.iter().cloned().map(BindArg::from).collect(),
        ))
    }

    pub fn predicate(&self) -> &str {
        &self.predicate
    }

    pub fn binds(&self) -> &[BindArg] {
        &self.binds
    }
}

#[derive(Debug, Clone)]
pub struct SelectClause(String);

impl SelectClause {
    pub fn new(expression: impl Into<String>) -> Self {
        Self(expression.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SelectClause {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SelectClause {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone)]
pub struct OrderClause {
    expression: String,
    descending: bool,
}

impl OrderClause {
    pub fn asc(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            descending: false,
        }
    }

    pub fn desc(expression: impl Into<String>) -> Self {
        Self {
            expression: expression.into(),
            descending: true,
        }
    }

    fn sql(&self) -> String {
        format!(
            "{} {}",
            self.expression,
            if self.descending { "DESC" } else { "ASC" }
        )
    }
}

#[derive(Debug, Clone)]
pub struct BoundQueryBuilder {
    selects: Vec<SelectClause>,
    from: SqlFragment,
    filters: Vec<FilterClause>,
    group_bys: Vec<String>,
    order_bys: Vec<OrderClause>,
    limit: Option<u64>,
    offset: Option<u64>,
}

impl BoundQueryBuilder {
    pub fn new(from: impl Into<String>) -> Self {
        Self {
            selects: Vec::new(),
            from: SqlFragment::new(from),
            filters: Vec::new(),
            group_bys: Vec::new(),
            order_bys: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    pub fn from_fragment(fragment: SqlFragment) -> Self {
        Self {
            selects: Vec::new(),
            from: fragment,
            filters: Vec::new(),
            group_bys: Vec::new(),
            order_bys: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    pub fn add_select(&mut self, select: impl Into<SelectClause>) {
        self.selects.push(select.into());
    }

    pub fn extend_selects<I>(&mut self, selects: I)
    where
        I: IntoIterator,
        I::Item: Into<SelectClause>,
    {
        self.selects.extend(selects.into_iter().map(Into::into));
    }

    pub fn add_filter(&mut self, filter: FilterClause) {
        self.filters.push(filter);
    }

    pub fn extend_filters<I>(&mut self, filters: I)
    where
        I: IntoIterator<Item = FilterClause>,
    {
        self.filters.extend(filters);
    }

    pub fn add_group_by(&mut self, expression: impl Into<String>) {
        self.group_bys.push(expression.into());
    }

    pub fn extend_group_bys<I>(&mut self, expressions: I)
    where
        I: IntoIterator,
        I::Item: Into<String>,
    {
        self.group_bys
            .extend(expressions.into_iter().map(Into::into));
    }

    pub fn add_order_by(&mut self, clause: OrderClause) {
        self.order_bys.push(clause);
    }

    pub fn set_limit(&mut self, limit: Option<u64>) {
        self.limit = limit;
    }

    pub fn set_offset(&mut self, offset: Option<u64>) {
        self.offset = offset;
    }

    pub fn sql(&self) -> String {
        let select_sql = self
            .selects
            .iter()
            .map(SelectClause::as_str)
            .collect::<Vec<_>>()
            .join(", ");

        let mut sql = format!("SELECT {select_sql} FROM {}", self.from.sql());

        if !self.filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(
                &self
                    .filters
                    .iter()
                    .map(FilterClause::predicate)
                    .collect::<Vec<_>>()
                    .join(" AND "),
            );
        }

        if !self.group_bys.is_empty() {
            sql.push_str(" GROUP BY ");
            sql.push_str(&self.group_bys.join(", "));
        }

        if !self.order_bys.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(
                &self
                    .order_bys
                    .iter()
                    .map(OrderClause::sql)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        if self.limit.is_some() {
            sql.push_str(" LIMIT ?");
        }

        if self.offset.is_some() {
            sql.push_str(" OFFSET ?");
        }

        sql
    }

    pub fn into_fragment(self) -> SqlFragment {
        let sql = self.sql();
        let mut binds = self.from.binds().to_vec();
        for filter in self.filters {
            binds.extend(filter.binds().iter().cloned());
        }
        if let Some(limit) = self.limit {
            binds.push(limit.into());
        }
        if let Some(offset) = self.offset {
            binds.push(offset.into());
        }
        SqlFragment::with_binds(sql, binds)
    }

    pub fn build(self, client: &Client) -> Query {
        self.into_fragment().into_query(client)
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundQueryBuilder, FilterClause, OrderClause};

    #[test]
    fn builder_keeps_bind_order_stable() {
        let mut builder = BoundQueryBuilder::new("analytics_domain_events");
        builder.add_select("route");
        builder.extend_filters([
            FilterClause::gte("created_at_ms", 10_i64),
            FilterClause::lte("created_at_ms", 20_i64),
            FilterClause::eq("merchant_id", "m_123"),
        ]);
        builder.add_order_by(OrderClause::desc("created_at_ms"));
        builder.set_limit(Some(25));
        builder.set_offset(Some(50));

        let fragment = builder.into_fragment();
        assert_eq!(
            fragment.sql(),
            "SELECT route FROM analytics_domain_events WHERE created_at_ms >= ? AND created_at_ms <= ? AND merchant_id = ? ORDER BY created_at_ms DESC LIMIT ? OFFSET ?"
        );
        assert_eq!(fragment.binds().len(), 5);
    }

    #[test]
    fn in_list_uses_placeholders() {
        let clause = FilterClause::in_list("gateway", &["adyen".to_string(), "stripe".to_string()])
            .expect("clause should exist");
        assert_eq!(clause.predicate(), "gateway IN (?, ?)");
        assert_eq!(clause.binds().len(), 2);
    }
}
