use std::{
    borrow::Cow,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{ILikeType, SqlBuilder, SqlValue, Wheres, str_type::Text};

pub trait SqlTable<'a> {
    fn table_expr(&self) -> SqlBuilder<'a>;
    fn alias(&self) -> &'a str;
}

#[derive(Clone, Debug)]
pub struct SqlField<'a> {
    pub alias: Option<&'a str>,
    pub table_alias: &'a str,
    pub field_name: &'static str,
}

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
impl<'a, T> DerefMut for SqlTypedField<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.field
    }
}

impl<'a, T> SqlTypedField<'a, T> {
    pub fn new(table_alias: &'a str, field_name: &'static str) -> Self {
        Self {
            field: SqlField {
                alias: None,
                table_alias,
                field_name,
            },
            value_type: PhantomData,
        }
    }

    pub fn with_alias(self, alias: &'a str) -> Self {
        Self {
            field: SqlField {
                alias: Some(alias),
                ..self.field
            },
            ..self
        }
    }

    pub fn with_table_alias(self, alias: &'a str) -> Self {
        Self {
            field: SqlField {
                table_alias: alias,
                ..self.field
            },
            ..self
        }
    }

    pub fn twn(&self) -> Cow<'a, str> {
        format!("{}.{}", self.table_alias, self.field_name).into()
    }

    pub fn erased(&self) -> SqlField<'a> {
        self.field.clone()
    }
}

impl<'a, T: 'a> SqlTypedField<'a, T>
where
    T: Into<SqlValue<'a>>,
{
    pub fn v_eq<V: Into<T>>(&self, v: V) -> Wheres<'a> {
        Wheres::equal(self.twn(), v.into())
    }

    pub fn v_in<V: Into<T>>(&self, vs: Vec<V>) -> Wheres<'a> {
        Wheres::r#in(self.twn(), vs.into_iter().map(|v| v.into()).collect())
    }
}

impl<'a> SqlTypedField<'a, Text> {
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
