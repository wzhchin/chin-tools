use crate::{ChinSqlError, DbType, IntoSqlSeg, PlaceHolderType};

use super::{SqlSeg, sql_value::SqlValue};

pub struct SqlInserter<'a> {
    table: &'a str,
    fields: Vec<(&'a str, SqlValue<'a>)>,
    extra: Vec<(&'a str, SqlValue<'a>)>,
    on_conflict: OnConflict,
}

#[derive(Default, Clone, Debug)]
pub enum OnConflict {
    Ignore,
    Replace(String),
    #[default]
    Default,
}

impl<'a> SqlInserter<'a> {
    pub fn new(table: &'static str) -> Self {
        SqlInserter {
            table,
            fields: vec![],
            extra: vec![],
            on_conflict: OnConflict::default(),
        }
    }

    pub fn table_name(mut self, table: &'static str) -> Self {
        self.table = table;
        self
    }

    pub fn field<T: Into<SqlValue<'a>>>(mut self, key: &'static str, value: T) -> Self {
        self.fields.push((key, value.into()));

        self
    }

    pub fn raw<T: Into<SqlValue<'a>>>(mut self, key: &'a str, value: Option<T>) -> Self {
        if let Some(v) = value {
            self.extra.push((key, v.into()));
        }

        self
    }

    pub fn on_conflict(mut self, on_conflict: OnConflict) -> Self {
        self.on_conflict = on_conflict;
        self
    }
}

impl<'a> IntoSqlSeg<'a> for SqlInserter<'a> {
    fn into_sql_seg2(
        self,
        db_type: DbType,
        pht: &mut PlaceHolderType,
    ) -> Result<SqlSeg<'a>, ChinSqlError> {
        if self.fields.is_empty() {
            return Err(ChinSqlError::BuilderSqlError(
                "insert files is empty".to_owned(),
            ));
        }

        let mut sql = String::new();
        sql.push_str("insert ");
        if matches!(db_type, DbType::Sqlite) {
            match self.on_conflict {
                OnConflict::Ignore => sql.push_str(" or ignore "),
                OnConflict::Replace(_) => sql.push_str(" or replace "),
                OnConflict::Default => {}
            }
        }
        sql.push_str(" into ");
        sql.push_str(self.table);
        sql.push('(');
        sql.push_str(
            self.fields
                .iter()
                .map(|(key, _)| *key)
                .collect::<Vec<&str>>()
                .join(",")
                .as_str(),
        );
        sql.push_str(") values (");

        let mut pht_vec = Vec::with_capacity(self.fields.len());
        for _ in self.fields.iter() {
            pht_vec.push(pht.next_ph());
        }

        sql.push_str(pht_vec.join(", ").as_str());
        sql.push(')');
        if matches!(db_type, DbType::Postgres) {
            match self.on_conflict {
                OnConflict::Ignore => sql.push_str(" ON CONFLICT DO NOTHING"),
                OnConflict::Replace(cond) => {
                    sql.push_str(" ON CONFLICT (");
                    sql.push_str(&cond);
                    sql.push_str(") DO UPDATE SET ");

                    sql.push_str(self.fields[0].0);
                    sql.push_str(" = ");
                    sql.push_str(pht_vec[0].as_str());

                    for (id, pht) in pht_vec.iter().enumerate().skip(1) {
                        sql.push_str(", ");
                        sql.push_str(self.fields[id].0);
                        sql.push('=');
                        sql.push_str(pht);
                    }
                }
                OnConflict::Default => {}
            }
        }

        let values = self.fields.into_iter().map(|e| e.1).collect();

        Ok(SqlSeg::of(sql, values))
    }
}
