use std::borrow::Cow;

use crate::{
    ChinSqlError, DbType, IntoSqlSeg, SqlField, SqlFieldTrait, SqlFields, SqlSeg, SqlTable,
    SqlTypedField,
};

use super::{place_hoder::PlaceHolderType, sql_value::SqlValue, wheres::Wheres};

pub trait CustomSqlSeg<'a>: Send {
    fn build(&self, value_type: &mut PlaceHolderType) -> Option<SqlSeg<'a>>;
}

enum SqlBuilderSeg<'a> {
    Comma(Vec<&'a str>),
    Str(Cow<'a, str>),
    Val(SqlValue<'a>),
    Custom(Box<dyn CustomSqlSeg<'a>>),
    Sub {
        alias: &'a str,
        query: SqlBuilder<'a>,
    },
    Where(Wheres<'a>),
    LimitOffset(LimitOffset),
}

pub struct SqlBuilder<'a> {
    segs: Vec<SqlBuilderSeg<'a>>,
}

impl<'a> Default for SqlBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> SqlBuilder<'a> {
    pub fn new() -> Self {
        Self { segs: vec![] }
    }

    pub fn read(table_name: &str, fields: &[&str]) -> Self {
        Self {
            segs: vec![SqlBuilderSeg::Str(
                format!("select {} from {} ", fields.join(", "), table_name).into(),
            )],
        }
    }

    pub fn read_all(table_name: &str) -> Self {
        Self {
            segs: vec![SqlBuilderSeg::Str(
                format!("select * from {table_name} ").into(),
            )],
        }
    }

    pub fn val<T: Into<SqlValue<'a>>>(mut self, val: T) -> Self {
        self.segs.push(SqlBuilderSeg::Val(val.into()));
        self
    }

    pub fn seg<T: Into<Cow<'a, str>>>(mut self, seg: T) -> Self {
        self.segs.push(SqlBuilderSeg::Str(seg.into()));
        self
    }

    pub fn some_then<T, F>(self, cond: Option<T>, trans: F) -> Self
    where
        F: FnOnce(T, Self) -> Self,
    {
        if let Some(t) = cond {
            trans(t, self)
        } else {
            self
        }
    }

    pub fn transform<F>(self, trans: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        trans(self)
    }

    pub fn r#where<T: Into<Wheres<'a>>>(mut self, wheres: T) -> Self {
        self.segs.push(SqlBuilderSeg::Where(wheres.into()));
        self
    }

    pub fn comma(mut self, values: Vec<&'a str>) -> Self {
        self.segs.push(SqlBuilderSeg::Comma(values));
        self
    }

    pub fn sub(mut self, alias: &'a str, query: SqlBuilder<'a>) -> Self {
        self.segs.push(SqlBuilderSeg::Sub { alias, query });
        self
    }

    pub fn custom<T: CustomSqlSeg<'a> + 'static>(mut self, custom: T) -> Self {
        self.segs.push(SqlBuilderSeg::Custom(Box::new(custom)));
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.segs
            .push(SqlBuilderSeg::LimitOffset(LimitOffset::new(limit)));
        self
    }

    pub fn limit_offset(mut self, limit: LimitOffset) -> Self {
        self.segs.push(SqlBuilderSeg::LimitOffset(limit));
        self
    }

    pub fn order_by<'b, T: Into<Vec<OrderBy<'b>>>>(self, orders: T) -> Self {
        let orders: Vec<String> = orders
            .into()
            .iter()
            .filter(|e| !matches!(e, OrderBy::None))
            .map(|e| match e {
                OrderBy::Asc(shared_str) => format!("{} asc", shared_str),
                OrderBy::Desc(shared_str) => format!("{} desc", shared_str),
                OrderBy::None => unreachable!(),
            })
            .collect();
        self.seg(" order by ").seg(orders.join(", "))
    }

    pub fn merge<SB: Into<SqlBuilder<'a>>>(mut self, other: SB) -> Self {
        let SqlBuilder { segs } = other.into();
        self.segs.extend(segs);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LimitOffset {
    pub limit: usize,
    pub offset: Option<usize>,
}

pub enum OrderBy<'a> {
    Asc(Cow<'a, str>),
    Desc(Cow<'a, str>),
    None,
}

impl LimitOffset {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            offset: None,
        }
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset.replace(offset);

        self
    }

    pub fn offset_if_some(mut self, offset: Option<usize>) -> Self {
        self.offset = offset;

        self
    }

    pub fn to_box(self) -> Box<dyn CustomSqlSeg<'static>> {
        Box::new(self)
    }
}

impl<'a> CustomSqlSeg<'a> for LimitOffset {
    fn build(&self, _: &mut PlaceHolderType) -> Option<SqlSeg<'a>> {
        match self.offset {
            Some(v) => Some(SqlSeg::of(
                format!("limit {} offset {}", self.limit, v),
                vec![],
            )),
            None => Some(SqlSeg::of(format!("limit {}", self.limit), vec![])),
        }
    }
}

impl<'a> IntoSqlSeg<'a> for SqlBuilder<'a> {
    fn into_sql_seg2(
        self,
        db_type: DbType,
        pht: &mut PlaceHolderType,
    ) -> Result<SqlSeg<'a>, ChinSqlError> {
        if self.segs.is_empty() {
            Err(ChinSqlError::BuilderSqlError("segs is empty".into()))?
        }

        let mut sb = String::new();
        let mut values: Vec<SqlValue<'a>> = Vec::new();

        for seg in self.segs {
            match seg {
                SqlBuilderSeg::Where(wr) => {
                    if let Some(ss) = wr.build(db_type, pht) {
                        sb.push_str(" where ");
                        sb.push_str(&ss.seg);
                        values.extend(ss.values)
                    }
                }
                SqlBuilderSeg::Comma(vs) => {
                    sb.push_str(vs.join(", ").as_str());
                }
                SqlBuilderSeg::Custom(custom) => {
                    if let Some(cs) = custom.build(pht) {
                        sb.push_str(&cs.seg);
                        values.extend(cs.values)
                    }
                }
                SqlBuilderSeg::Sub { alias, query } => {
                    if let Ok(s) = query.into_sql_seg2(db_type, pht) {
                        sb.push_str(" (");
                        sb.push_str(&s.seg);
                        sb.push_str(") ");
                        sb.push_str(alias);
                        values.extend(s.values);
                    }
                }
                SqlBuilderSeg::Str(s) => {
                    sb.push_str(&s);
                    sb.push(' ');
                }
                SqlBuilderSeg::Val(val) => {
                    sb.push_str(&pht.next_ph());
                    sb.push(' ');
                    values.push(val);
                }
                SqlBuilderSeg::LimitOffset(limit_offset) => {
                    let SqlSeg { seg, values: vs } =
                        limit_offset.build(pht).ok_or(ChinSqlError::TransformError(
                            "Unable convert limit offset to sql seg.".to_owned(),
                        ))?;
                    sb.push_str(&seg);
                    values.extend(vs);
                }
            };
            if !sb.ends_with(" ") {
                sb.push(' ');
            }
        }

        Ok(SqlSeg::of(sb, values))
    }
}

impl<'a> From<&'a str> for SqlBuilder<'a> {
    fn from(value: &'a str) -> Self {
        Self {
            segs: vec![SqlBuilderSeg::Str(value.into())],
        }
    }
}

pub enum JoinType {
    LeftJoin,
    InnerJoin,
    RightJoin,
}

pub struct JoinCond<'a> {
    l_table: &'a str,
    l_field: &'a str,
    r_table: &'a str,
    r_field: &'a str,
}

impl<'a, T> From<(SqlTypedField<'a, T>, SqlTypedField<'a, T>)> for JoinCond<'a> {
    fn from(value: (SqlTypedField<'a, T>, SqlTypedField<'a, T>)) -> Self {
        JoinCond {
            l_table: value.0.table_alias(),
            l_field: value.0.field_name(),
            r_table: value.1.table_alias(),
            r_field: value.1.field_name(),
        }
    }
}
pub struct JoinTable<'a> {
    pub join_type: JoinType,
    pub table: Froms<'a>,
    pub conds: Vec<JoinCond<'a>>,
}

pub struct Joins<'a> {
    pub base: Froms<'a>,
    pub joins: Vec<JoinTable<'a>>,
}

impl<'a> Joins<'a> {
    pub fn new(base: Froms<'a>) -> Self {
        Self {
            base,
            joins: vec![],
        }
    }

    pub fn join<T: Into<Option<JoinTable<'a>>>>(mut self, table: T) -> Self {
        let jt = table.into();
        if let Some(jt) = jt {
            self.joins.push(jt);
        }
        self
    }

    pub fn join_if<T: Into<Option<JoinTable<'a>>>>(self, flag: bool, table: T) -> Self {
        if flag { self.join(table.into()) } else { self }
    }

    pub fn join_some<V, F>(self, v: Option<V>, map: F) -> Self
    where
        F: Fn(V) -> JoinTable<'a>,
    {
        match v {
            Some(v) => self.join(map(v)),
            None => self,
        }
    }
}

impl<'a> From<&Joins<'a>> for SqlBuilder<'a> {
    fn from(value: &Joins<'a>) -> Self {
        let mut sql_builder = SqlBuilder::new();
        sql_builder = sql_builder.merge(&value.base);
        for table in &value.joins {
            let JoinTable {
                join_type,
                table,
                conds,
            } = table;

            match join_type {
                JoinType::LeftJoin => {
                    sql_builder = sql_builder.seg("left join");
                }
                JoinType::InnerJoin => {
                    sql_builder = sql_builder.seg("inner join");
                }
                JoinType::RightJoin => {
                    sql_builder = sql_builder.seg("right join");
                }
            };
            sql_builder = sql_builder.merge(&*table).seg("on");
            let cond_len = conds.len();
            for (i, join_cond) in conds.into_iter().enumerate() {
                sql_builder = sql_builder
                    .seg(join_cond.l_table)
                    .seg(".")
                    .seg(join_cond.l_field)
                    .seg("=")
                    .seg(join_cond.r_table)
                    .seg(".")
                    .seg(join_cond.r_field);
                if i < cond_len - 1 {
                    sql_builder = sql_builder.seg("and");
                }
            }
        }

        sql_builder
    }
}

pub trait Fields<'a> {
    fn to_select_fields(&self) -> String;
}

impl<'a> Fields<'a> for String {
    fn to_select_fields(&self) -> String {
        self.to_string()
    }
}

pub enum Froms<'a> {
    Table {
        table_name: &'a str,
        alias: Option<&'a str>,
    },
    SubQuery {
        table: Box<SqlReader<'a>>,
        alias: &'a str,
    },
    Joins(Box<Joins<'a>>),
}

impl<'a> From<&'a str> for Froms<'a> {
    fn from(value: &'a str) -> Self {
        Self::Table {
            table_name: value,
            alias: None,
        }
    }
}

impl<'a> From<Joins<'a>> for Froms<'a> {
    fn from(value: Joins<'a>) -> Self {
        Self::Joins(Box::new(value))
    }
}

impl<'a> From<&Froms<'a>> for SqlBuilder<'a> {
    fn from(value: &Froms<'a>) -> Self {
        match value {
            Froms::Table { table_name, alias } => SqlBuilder::new()
                .seg(*table_name)
                .some_then(*alias, |e, v| v.seg("as").seg(e)),
            Froms::SubQuery { table, alias } => SqlBuilder::new()
                .seg("(")
                .merge(&*(*table))
                .seg(") as")
                .seg(*alias),
            Froms::Joins(joins) => (&**joins).into(),
        }
    }
}

impl<'a, T> From<&T> for Froms<'a>
where
    T: SqlTable<'a>,
{
    fn from(value: &T) -> Self {
        match value.table_expr() {
            crate::SqlTableExpr::Plain(plain) => Froms::Table {
                table_name: plain,
                alias: Some(value.alias()),
            },
            crate::SqlTableExpr::Expr(sql_reader) => Froms::SubQuery {
                table: sql_reader.into(),
                alias: value.alias(),
            },
        }
    }
}

#[derive(Debug, Default)]
pub enum GroupBy<'a> {
    Plain(Vec<Cow<'a, str>>),
    #[default]
    None,
}

#[derive(Debug, Default)]
pub enum Having<'a> {
    Custom(Cow<'a, str>),
    #[default]
    None,
}

pub enum SqlReader<'a> {
    Union(Vec<SqlReader<'a>>),
    Single(SqlSingleReader<'a>),
}

pub struct SqlSingleReader<'a> {
    fields: SqlFields<'a>,
    froms: Froms<'a>,
    wheres: Wheres<'a>,
    group_by: GroupBy<'a>,
    having: Having<'a>,
    order_by: Option<Vec<OrderBy<'a>>>,
    limit: Option<LimitOffset>,
}

impl<'a> SqlReader<'a> {
    pub fn read<V, R>(fields: V, froms: R) -> SqlReaderBuilder<'a>
    where
        V: Into<SqlFields<'a>>,
        R: Into<Froms<'a>>,
    {
        SqlReaderBuilder {
            reader: SqlSingleReader {
                fields: fields.into(),
                froms: froms.into(),
                wheres: Wheres::None,
                order_by: None,
                limit: None,
                group_by: Default::default(),
                having: Default::default(),
            },
        }
    }

    pub fn limit(self, limit: LimitOffset) -> Self {
        match self {
            SqlReader::Union(sql_readers) => SqlReader::Single(SqlSingleReader {
                fields: SqlFields(vec![SqlField {
                    alias: None,
                    inner: crate::SqlFieldInner::Plain {
                        table_alias: "t",
                        field_name: "*",
                    }
                    .into(),
                }]),
                froms: Froms::SubQuery {
                    table: Box::new(SqlReader::Union(sql_readers)),
                    alias: "t",
                },
                wheres: Wheres::None,
                group_by: GroupBy::None,
                having: Having::None,
                order_by: None,
                limit: Some(limit),
            }),
            SqlReader::Single(mut sql_single_reader) => {
                sql_single_reader.limit.replace(limit);
                SqlReader::Single(sql_single_reader)
            }
        }
    }
}

pub struct SqlReaderBuilder<'a> {
    reader: SqlSingleReader<'a>,
}

impl<'a> SqlReaderBuilder<'a> {
    pub fn wheres(mut self, wheres: Wheres<'a>) -> Self {
        self.reader.wheres = wheres;
        self
    }

    pub fn order_by<T: Into<Vec<OrderBy<'a>>>>(mut self, orders: T) -> Self {
        self.reader.order_by.replace(orders.into());
        self
    }

    pub fn limit(mut self, limit: LimitOffset) -> Self {
        self.reader.limit.replace(limit);
        self
    }

    pub fn group_by<T: Into<GroupBy<'a>>>(mut self, group_by: T) -> Self {
        self.reader.group_by = group_by.into();
        self
    }

    pub fn having<T: Into<Having<'a>>>(mut self, having: T) -> Self {
        self.reader.having = having.into();
        self
    }

    pub fn build(self) -> SqlSingleReader<'a> {
        self.reader
    }

    pub fn build2(self) -> SqlReader<'a> {
        SqlReader::Single(self.reader)
    }
}

impl<'a> From<&SqlSingleReader<'a>> for SqlBuilder<'a> {
    fn from(value: &SqlSingleReader<'a>) -> Self {
        let select = value
            .fields
            .0
            .iter()
            .map(|m| match m.alias {
                Some(alias) => {
                    format!("{}.{} as {}", m.table_alias(), m.field_name(), alias)
                }
                None => {
                    format!("{}.{}", m.table_alias(), m.field_name())
                }
            })
            .collect::<Vec<String>>()
            .join(", ");
        SqlBuilder::new()
            .seg("select")
            .seg(select)
            .seg("from")
            .merge(&value.froms)
            .r#where(value.wheres.clone())
            .transform(|this| match &value.group_by {
                GroupBy::Plain(cows) => this.seg("order by").seg(cows.join(", ")),
                GroupBy::None => this,
            })
            .transform(|this| match &value.having {
                Having::Custom(cow) => this.seg("having").seg(cow.clone()),
                Having::None => this,
            })
            .transform(|this| match &value.order_by {
                Some(order_by) => {
                    let c: Vec<String> = order_by
                        .iter()
                        .filter_map(|ob| match ob {
                            OrderBy::Asc(cow) => Some(format!("{} asc", cow)),
                            OrderBy::Desc(cow) => Some(format!("{} desc", cow)),
                            OrderBy::None => None,
                        })
                        .collect();
                    if !c.is_empty() {
                        this.seg("order by").seg(c.join(", "))
                    } else {
                        this
                    }
                }
                None => this,
            })
            .transform(|this| match value.limit {
                Some(lo) => this.limit_offset(lo),
                None => this,
            })
    }
}

impl<'a> From<SqlSingleReader<'a>> for SqlReader<'a> {
    fn from(value: SqlSingleReader<'a>) -> Self {
        SqlReader::Single(value)
    }
}

impl<'a> IntoSqlSeg<'a> for SqlSingleReader<'a> {
    fn into_sql_seg2(
        self,
        db_type: DbType,
        pht: &mut PlaceHolderType,
    ) -> Result<SqlSeg<'a>, ChinSqlError> {
        let sb: SqlReader<'a> = self.into();
        let sb: SqlBuilder<'a> = (&sb).into();
        sb.into_sql_seg2(db_type, pht)
    }
}

impl<'a> IntoSqlSeg<'a> for SqlReader<'a> {
    fn into_sql_seg2(
        self,
        db_type: DbType,
        pht: &mut PlaceHolderType,
    ) -> Result<SqlSeg<'a>, ChinSqlError> {
        match self {
            SqlReader::Union(sql_readers) => {
                let mut values = vec![];
                let mut segs = vec![];
                for sr in sql_readers {
                    let ss = sr.into_sql_seg2(db_type, pht)?;
                    segs.push(ss.seg);
                    values.extend(ss.values);
                }
                Ok(SqlSeg {
                    seg: segs.join(" union "),
                    values,
                })
            }
            SqlReader::Single(sql_reader) => sql_reader.into_sql_seg2(db_type, pht),
        }
    }
}

impl<'a> From<&SqlReader<'a>> for SqlBuilder<'a> {
    fn from(value: &SqlReader<'a>) -> Self {
        match value {
            SqlReader::Union(sql_readers) => {
                let mut sql_builder = SqlBuilder::new();
                let mut union = false;
                for fr in sql_readers {
                    if union {
                        sql_builder = sql_builder.seg("union")
                    } else {
                        union = true;
                    }
                    sql_builder = sql_builder.merge(fr);
                }
                sql_builder
            }
            SqlReader::Single(sql_single_reader) => sql_single_reader.into(),
        }
    }
}
