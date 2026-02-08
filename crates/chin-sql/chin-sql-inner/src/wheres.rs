use std::borrow::Cow;

use crate::{DbType, PlaceHolderType, SegOrVal, SqlSeg};

use super::sql_value::SqlValue;

#[derive(Clone, Debug)]
pub enum WhereConjOp {
    And,
    Or,
}

pub enum ILikeType {
    Original,
    RightFuzzy,
    LeftFuzzy,
    Fuzzy,
}

pub struct FilterCount {
    pub check_filter_count: bool,
    pub filter_count: usize,
}

impl Default for FilterCount {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterCount {
    pub fn new() -> Self {
        Self {
            check_filter_count: true,
            filter_count: 0,
        }
    }

    pub fn increament(self) -> Self {
        Self {
            filter_count: self.filter_count + 1,
            ..self
        }
    }
    pub fn check(&self) -> bool {
        self.check_filter_count && self.filter_count > 0
    }
}

#[derive(Clone, Debug)]
pub enum Wheres<'a> {
    Conj(WhereConjOp, Vec<Wheres<'a>>),
    In(Cow<'a, str>, Vec<SqlValue<'a>>),
    Not(Box<Wheres<'a>>),
    Compare {
        key: Cow<'a, str>,
        operator: Cow<'a, str>,
        value: SqlValue<'a>,
    }, // key, operator, value
    Raw(Cow<'a, str>),
    SOV(Vec<SegOrVal<'a>>),
    IIike {
        key: Cow<'a, str>,
        value: String,
    },
    None,
}

impl<'a> Wheres<'a> {
    pub fn empty(&self) -> bool {
        match self {
            Wheres::Conj(_where_conj_op, items) => {
                items.is_empty() || items.iter().all(|e| e.empty())
            }
            Wheres::In(_cow, sql_values) => sql_values.is_empty(),
            Wheres::Not(wheres) => wheres.empty(),
            Wheres::Compare {
                key,
                operator: _,
                value: _,
            } => key.is_empty(),
            Wheres::Raw(cow) => cow.is_empty(),
            Wheres::SOV(seg_or_vals) => seg_or_vals.is_empty(),
            Wheres::IIike { key, value: _ } => key.is_empty(),
            Wheres::None => true,
        }
    }

    pub fn equal<T: Into<SqlValue<'a>>, S: Into<Cow<'a, str>>>(key: S, v: T) -> Self {
        Self::Compare {
            key: key.into(),
            operator: "=".into(),
            value: v.into(),
        }
    }

    pub fn ilike<T: AsRef<str>, S: Into<Cow<'a, str>>>(key: S, v: T, exact: ILikeType) -> Self {
        let s = v.as_ref();
        if s.is_empty() {
            return Wheres::None;
        }
        Self::IIike {
            key: key.into(),
            value: match exact {
                ILikeType::Original => v.as_ref().into(),
                ILikeType::RightFuzzy => format!("{}%", v.as_ref()),
                ILikeType::LeftFuzzy => format!("%{}", v.as_ref()),
                ILikeType::Fuzzy => format!("%{}%", v.as_ref()),
            },
        }
    }
    pub fn is_null<S: Into<Cow<'a, str>>>(key: S) -> Self {
        Self::compare_str(key.into(), "is", "null")
    }

    pub fn is_not_null<S: Into<Cow<'a, str>>>(key: S) -> Self {
        Self::compare_str(key.into(), "is not", "null")
    }

    pub fn compare<SK: Into<Cow<'a, str>>, SO: Into<Cow<'a, str>>, T: Into<SqlValue<'a>>>(
        key: SK,
        operator: SO,
        v: T,
    ) -> Self {
        Self::Compare {
            key: key.into(),
            operator: operator.into(),
            value: v.into(),
        }
    }

    pub fn compare_str<T: AsRef<str>, S: Into<Cow<'a, str>>>(
        key: S,
        operator: &'a str,
        v: T,
    ) -> Self {
        Self::Raw(Cow::Owned(format!(
            "{} {} {}",
            key.into(),
            operator,
            v.as_ref()
        )))
    }

    pub fn if_some<T, F>(original: Option<T>, map: F) -> Self
    where
        F: FnOnce(T) -> Self,
    {
        match original {
            Some(t) => map(t),
            None => Wheres::None,
        }
    }

    pub fn not<T: Into<Wheres<'a>>>(values: T) -> Self {
        Self::Not(Box::new(values.into()))
    }

    pub fn and<T: Into<Vec<Wheres<'a>>>>(values: T) -> Self {
        Self::Conj(WhereConjOp::And, values.into())
    }

    pub fn or<T: Into<Vec<Wheres<'a>>>>(values: T) -> Self {
        Self::Conj(WhereConjOp::Or, values.into())
    }

    pub fn transform<T, F>(original: T, map: F) -> Self
    where
        F: FnOnce(T) -> Self,
    {
        map(original)
    }

    pub fn r#in<T: Into<SqlValue<'a>>, S: Into<Cow<'a, str>>>(key: S, values: Vec<T>) -> Self {
        Self::In(key.into(), values.into_iter().map(|e| e.into()).collect())
    }

    pub fn none() -> Self {
        Self::None
    }

    pub fn build(self, db_type: DbType, value_type: &mut PlaceHolderType) -> Option<SqlSeg<'a>> {
        let mut seg = String::new();
        let mut values: Vec<SqlValue<'a>> = Vec::new();

        match self {
            Wheres::Conj(op, fs) => {
                let vs: Vec<String> = fs
                    .into_iter()
                    .filter_map(|e| {
                        e.build(db_type, value_type).map(|ss| {
                            values.extend(ss.values);
                            ss.seg
                        })
                    })
                    .collect();
                if vs.is_empty() {
                    return None;
                }
                let op = match op {
                    WhereConjOp::And => " and ",
                    WhereConjOp::Or => " or ",
                };

                seg.push_str(vs.join(op).as_str())
            }
            Wheres::In(key, fs) => {
                log::info!("print: {key:?}, {fs:?}");
                seg.push_str(key.as_ref());
                seg.push_str(" in (");
                let vs = fs
                    .iter()
                    .map(|_| value_type.next_ph())
                    .collect::<Vec<String>>();
                seg.push_str(vs.join(",").as_str());

                seg.push(')');
                values.extend(fs)
            }
            Wheres::Not(fs) => {
                seg.push_str(" not ( ");
                if let Some(ss) = fs.build(db_type, value_type) {
                    seg.push_str(&ss.seg);
                    seg.push(')');

                    values.extend(ss.values);
                } else {
                    return None;
                }
            }
            Wheres::None => {
                return None;
            }
            Wheres::Compare {
                key,
                operator,
                value,
            } => {
                seg.push_str(key.as_ref());
                seg.push(' ');
                seg.push_str(operator.as_ref());
                seg.push(' ');

                seg.push_str(&value_type.next_ph());
                values.push(value);
            }
            Wheres::Raw(cow) => {
                if !seg.ends_with(" ") {
                    seg.push(' ');
                }
                seg.push_str(cow.as_ref());

                seg.push(' ');
            }
            Wheres::SOV(seg_or_vals) => {
                for sov in seg_or_vals {
                    match sov {
                        SegOrVal::Str(cow) => {
                            seg.push_str(&cow);
                        }
                        SegOrVal::Val(sql_value) => {
                            seg.push_str(&value_type.next_ph());
                            values.push(sql_value);
                        }
                    }
                }
            }
            Wheres::IIike { key, value } => {
                let ilike = match db_type {
                    DbType::Sqlite => Self::Compare {
                        key,
                        operator: "like".into(),
                        value: value.into(),
                    },
                    DbType::Postgres => Self::Compare {
                        key,
                        operator: "ilike".into(),
                        value: value.into(),
                    },
                };
                let s = ilike.build(db_type, value_type);
                if let Some(SqlSeg { seg: s, values: v }) = s {
                    seg.push_str(s.as_str());
                    values.extend(v);
                }
            }
        }

        Some(SqlSeg::of(seg, values))
    }
}
