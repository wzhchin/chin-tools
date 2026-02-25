use std::{
    borrow::Cow,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{ILikeType, SqlReader, SqlValue, Wheres, str_type::{Text, Varchar}};

pub enum SqlTableExpr<'a> {
    Plain(&'a str),
    Expr(SqlReader<'a>),
}

pub trait SqlTable<'a> {
    fn table_expr(&self) -> SqlTableExpr<'a>;
    fn alias(&self) -> &'a str;
    #[inline]
    fn all_fields(&self) -> SqlField<'a> {
        SqlField {
            alias: None,
            inner: SqlFieldInner::Plain {
                table_alias: self.alias(),
                field_name: "*",
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct SqlField<'a> {
    pub alias: Option<&'a str>,
    pub inner: SqlFieldInner<'a>,
}

impl<'a> SqlField<'a> {
    pub(crate) fn to_select_field(&self) -> String {
        let mut sb = String::new();
        match &self.inner {
            SqlFieldInner::Plain {
                table_alias,
                field_name,
            } => {
                sb.push_str(*table_alias);
                sb.push('.');
                sb.push_str(field_name);
            }
            SqlFieldInner::Raw { expr } => {
                sb.push_str(*expr);
            }
        }

        if let Some(alias) = self.alias {
            sb.push_str(" as ");
            sb.push_str(alias);
        }

        sb
    }
}

#[derive(Clone, Debug)]
pub enum SqlFieldInner<'a> {
    Plain {
        table_alias: &'a str,
        field_name: &'static str,
    },
    Raw {
        expr: &'a str,
    },
}

impl<'a> SqlFieldInner<'a> {
    pub fn with_table_alias(self, alias: &'a str) -> Self {
        match self {
            SqlFieldInner::Plain {
                table_alias: _,
                field_name,
            } => SqlFieldInner::Plain {
                table_alias: alias,
                field_name,
            },
            SqlFieldInner::Raw { expr } => SqlFieldInner::Raw { expr },
        }
    }
}

#[derive(Clone)]
pub struct SqlTypedField<'a, T> {
    field: SqlField<'a>,
    value_type: PhantomData<T>,
}

impl<'a, T> Deref for SqlTypedField<'a, T> {
    type Target = SqlField<'a>;

    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<'a, T> From<SqlTypedField<'a, T>> for SqlField<'a> {
    fn from(value: SqlTypedField<'a, T>) -> Self {
        value.field
    }
}

pub struct SqlFields<'a>(pub(crate) Vec<SqlField<'a>>);

impl<'a, T1> From<T1> for SqlFields<'a>
where
    T1: Into<SqlField<'a>>,
{
    fn from(value: T1) -> Self {
        SqlFields(vec![value.into()])
    }
}

impl<'a, T1, T2, T3, T4> From<(T1, T2, T3, T4)> for SqlFields<'a>
where
    T1: Into<SqlField<'a>>,
    T2: Into<SqlField<'a>>,
    T3: Into<SqlField<'a>>,
    T4: Into<SqlField<'a>>,
{
    fn from(value: (T1, T2, T3, T4)) -> Self {
        SqlFields(vec![
            value.0.into(),
            value.1.into(),
            value.2.into(),
            value.3.into(),
        ])
    }
}

impl<'a, T1, T2> From<(T1, T2)> for SqlFields<'a>
where
    T1: Into<SqlField<'a>>,
    T2: Into<SqlField<'a>>,
{
    fn from(value: (T1, T2)) -> Self {
        SqlFields(vec![value.0.into(), value.1.into()])
    }
}

impl<'a, T1, T2, T3> From<(T1, T2, T3)> for SqlFields<'a>
where
    T1: Into<SqlField<'a>>,
    T2: Into<SqlField<'a>>,
    T3: Into<SqlField<'a>>,
{
    fn from(value: (T1, T2, T3)) -> Self {
        SqlFields(vec![value.0.into(), value.1.into(), value.2.into()])
    }
}

impl<'a> From<Vec<SqlField<'a>>> for SqlFields<'a> {
    fn from(value: Vec<SqlField<'a>>) -> Self {
        SqlFields(value)
    }
}

impl<'a> From<&[SqlField<'a>]> for SqlFields<'a> {
    fn from(value: &[SqlField<'a>]) -> Self {
        SqlFields(value.into())
    }
}

pub trait SqlFieldTrait<'a>: Sync + Send {
    fn alias(&self) -> Option<&'a str>;
    fn table_alias(&self) -> &'a str;
    fn field_name(&self) -> &'static str;
}

impl<'a> SqlFieldTrait<'a> for SqlField<'a> {
    #[inline]
    fn alias(&self) -> Option<&'a str> {
        self.alias.clone()
    }

    #[inline]
    fn table_alias(&self) -> &'a str {
        match self.inner {
            SqlFieldInner::Plain {
                table_alias,
                field_name: _,
            } => table_alias,
            SqlFieldInner::Raw { expr: _ } => unreachable!(),
        }
    }

    #[inline]
    fn field_name(&self) -> &'static str {
        match self.inner {
            SqlFieldInner::Plain {
                table_alias: _,
                field_name,
            } => field_name,
            SqlFieldInner::Raw { expr: _ } => unreachable!(),
        }
    }
}

impl<'a, T> From<&SqlTypedField<'a, T>> for SqlField<'a> {
    fn from(value: &SqlTypedField<'a, T>) -> Self {
        value.field.clone()
    }
}

impl<'a> From<&'a dyn SqlFieldTrait<'a>> for SqlField<'a> {
    fn from(value: &'a dyn SqlFieldTrait<'a>) -> Self {
        Self {
            alias: value.alias(),
            inner: SqlFieldInner::Plain {
                table_alias: value.table_alias(),
                field_name: value.field_name(),
            },
        }
    }
}

impl<'a, T> DerefMut for SqlTypedField<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

impl<'a, T> SqlTypedField<'a, T> {
    #[inline]
    pub fn new(table_alias: &'a str, field_name: &'static str) -> Self {
        Self {
            field: SqlField {
                alias: None,
                inner: SqlFieldInner::Plain {
                    table_alias: table_alias,
                    field_name: field_name,
                },
            },
            value_type: PhantomData,
        }
    }

    #[inline]
    pub fn with_alias(self, alias: &'a str) -> Self {
        Self {
            field: SqlField {
                alias: Some(alias),
                ..self.field
            },
            ..self
        }
    }

    #[inline]
    pub fn alias(&self) -> &'a str {
        self.alias.unwrap_or(match &self.inner {
            SqlFieldInner::Plain {
                table_alias: _,
                field_name,
            } => field_name,
            SqlFieldInner::Raw { expr: _ } => unreachable!(),
        })
    }

    #[inline]
    pub fn with_table_alias(mut self, alias: &'a str) -> Self {
        self.alias.replace(alias);
        self
    }

    #[inline]
    pub fn twn(&self) -> Cow<'a, str> {
        format!("{}.{}", self.table_alias(), self.field_name()).into()
    }

    #[inline]
    pub fn erased(&self) -> SqlField<'a> {
        self.field.clone()
    }

    #[inline]
    pub fn v_is_null(&self) -> Wheres<'a> {
        Wheres::is_null(self.twn())
    }
}

impl<'a, T: 'a> SqlTypedField<'a, T>
where
    T: Into<SqlValue<'a>>,
{
    pub fn v_eq<V: Into<T>>(&self, v: V) -> Wheres<'a> {
        Wheres::equal(self.twn(), v.into())
    }

    pub fn v_in<S, V>(&self, vs: S) -> Wheres<'a>
    where
        S: Into<Vec<V>>,
        V: Into<T>,
    {
        Wheres::r#in(
            self.twn(),
            vs.into().into_iter().map(|e| e.into()).collect(),
        )
    }
}

impl<'a> SqlTypedField<'a, Text> {
    pub fn v_ilike<V: AsRef<str>>(&self, v: V, exact: ILikeType) -> Wheres<'a> {
        Wheres::ilike(self.twn(), v.as_ref(), exact)
    }
}

impl<'a, const LIMIT: usize> SqlTypedField<'a, Varchar<LIMIT>> {
    pub fn v_ilike<V: AsRef<str>>(&self, v: V, exact: ILikeType) -> Wheres<'a> {
        Wheres::ilike(self.twn(), v.as_ref(), exact)
    }
}

impl<'a> SqlTypedField<'a, i64> {
    pub fn v_gt<V: Into<i64>>(&self, v: V) -> Wheres<'a> {
        Wheres::compare(self.twn(), ">", v.into())
    }

    pub fn v_lt<V: Into<i64>>(&self, v: V) -> Wheres<'a> {
        Wheres::compare(self.twn(), "<", v.into())
    }

    pub fn v_ge<V: Into<i64>>(&self, v: V) -> Wheres<'a> {
        Wheres::compare(self.twn(), ">=", v.into())
    }

    pub fn v_le<V: Into<i64>>(&self, v: V) -> Wheres<'a> {
        Wheres::compare(self.twn(), "<=", v.into())
    }
}
