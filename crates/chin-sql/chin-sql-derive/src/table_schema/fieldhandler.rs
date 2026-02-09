use std::collections::HashMap;

use chin_sql::LogicFieldType;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Field, PathArguments, Type, TypePath};

#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyOrder {
    Default,
    Num(u16),
}

impl KeyOrder {
    pub(crate) fn order(&self) -> u16 {
        match self {
            KeyOrder::Default => 0,
            KeyOrder::Num(u) => *u,
        }
    }
}

#[derive(Debug)]
pub(crate) struct FieldInfo {
    pub column_name: String,
    pub field_type: LogicFieldType,
    pub not_null: bool,
    pub key_map: HashMap<String, (bool, KeyOrder)>, // key_name(lower), unique?, keyorder
    pub pkey: Option<KeyOrder>,
    pub to_sql_func: Option<String>,
}

pub(crate) fn parse_field_info(field: &Field) -> Result<FieldInfo, syn::Error> {
    let field_name = field.ident.as_ref().unwrap();
    let column_name = field_name.to_string().to_lowercase().replace("\"", "");

    let (field_type, not_null) = match &field.ty {
        Type::Path(type_path) => parse_field_type(field, type_path),
        Type::Group(group) => match group.elem.as_ref() {
            Type::Path(type_path) => parse_field_type(field, type_path),
            v => Err(syn::Error::new(
                field.span(),
                format!(
                    "Error compling, not ty type in group {:#?}, true is {:#?}",
                    field.to_token_stream().to_string(),
                    v
                ),
            )),
        },
        v => Err(syn::Error::new(
            field.span(),
            format!(
                "Error compling, not ty type {:#?}, true is {:#?}",
                field.to_token_stream().to_string(),
                v
            ),
        )),
    }?;

    let pkey = find_pkey(field)?;
    let key_map = find_attr_key(&column_name, field)?;
    let to_sql_func = find_to_sql_func(field)?;

    Ok(FieldInfo {
        column_name,
        field_type,
        not_null,
        key_map,
        pkey,
        to_sql_func,
    })
}

fn parse_field_type(
    field: &Field,
    type_path: &TypePath,
) -> Result<(chin_sql::LogicFieldType, bool), syn::Error> {
    if let Some(segment) = type_path.path.segments.last() {
        let nullable = segment.ident.to_string().as_str() == "Option";
        let rt = find_attr_alias_type(field);
        let raw_rust_type = if let Some(Ok(rt)) = rt {
            rt
        } else if nullable {
            if let PathArguments::AngleBracketed(ab) = &segment.arguments {
                ab.args.to_token_stream().to_string()
            } else {
                return Err(syn::Error::new(
                    field.span(),
                    format!(
                        "This field is optional, but there is not Generic Type {:#?}",
                        field.to_token_stream().to_string()
                    ),
                ));
            }
        } else {
            segment.to_token_stream().to_string()
        };

        let raw_rust_type = raw_rust_type.replace(" ", "").replace("\"", "");

        let sql_type = match raw_rust_type.as_str() {
            "Text" => chin_sql::LogicFieldType::Text,
            "i32" => chin_sql::LogicFieldType::I32,
            "i64" => chin_sql::LogicFieldType::I64,
            "f32" => chin_sql::LogicFieldType::F64,
            "f64" => chin_sql::LogicFieldType::F64,
            "bool" => chin_sql::LogicFieldType::Bool,
            "DateTime<FixedOffset>" => chin_sql::LogicFieldType::Timestamptz,
            "DateTime<Utc>" => chin_sql::LogicFieldType::Timestamp,
            rt => {
                if rt.starts_with("Varchar<") && rt.ends_with(">") {
                    let text = &rt[8..(rt.len() - 1)];
                    let bound = text.parse::<u16>().map_err(|err| {
                        syn::Error::new(field.span(), format!("{text} in `{rt}` is illegal, {err}"))
                    })?;
                    chin_sql::LogicFieldType::Varchar(bound)
                } else {
                    Err(syn::Error::new(
                        field.span(),
                        format!("Unkown Rust Type {:#?}", raw_rust_type.as_str()),
                    ))?
                }
            }
        };

        // This is the corrected `quote!` block
        let not_null = !nullable;
        Ok((sql_type, not_null))
    } else {
        Err(syn::Error::new(
            field.span(),
            format!(
                "Error compling, cannot find the field ident {:#?}",
                field.to_token_stream().to_string()
            ),
        ))
    }
}

pub(crate) fn find_attr_key(
    column_name: &str,
    field: &Field,
) -> Result<HashMap<String, (bool, KeyOrder)>, syn::Error> {
    let mut map = HashMap::new();
    for attr in &field.attrs {
        let unique = if attr.path().is_ident("gts_key") {
            false
        } else if attr.path().is_ident("gts_unique") {
            true
        } else {
            continue;
        };

        let meta = &attr.meta;
        if let syn::Meta::NameValue(name_value) = meta {
            if let syn::Expr::Lit(lit) = &name_value.value
                && let syn::Lit::Str(lit_str) = &lit.lit
            {
                let value = lit_str.value();
                let cs: Vec<&str> = value.split(":").collect();
                let key = cs.first().unwrap().to_lowercase();
                let order = cs
                    .get(1)
                    .map(|e| e.parse::<u16>().map(KeyOrder::Num))
                    .unwrap_or(Ok(KeyOrder::Default))
                    .map_err(|_| {
                        syn::Error::new(field.span(), "form should look like key_name[:0]")
                    })?;

                map.insert(key, (unique, order));
            }
        } else {
            map.insert(column_name.to_owned(), (unique, KeyOrder::Default));
        }
    }

    Ok(map)
}

fn find_pkey(field: &Field) -> Result<Option<KeyOrder>, syn::Error> {
    for attr in &field.attrs {
        if attr.path().is_ident("gts_primary") {
            let meta = &attr.meta;
            if let syn::Meta::NameValue(name_value) = meta {
                if let syn::Expr::Lit(lit) = &name_value.value
                    && let syn::Lit::Int(lit_int) = &lit.lit
                {
                    return Ok(Some(KeyOrder::Num(lit_int.base10_parse()?)));
                }
            } else {
                return Ok(Some(KeyOrder::Default));
            }
        }
    }

    Ok(None)
}

fn find_to_sql_func(field: &Field) -> Result<Option<String>, syn::Error> {
    for attr in &field.attrs {
        if attr.path().is_ident("gts_tosql") {
            let meta = &attr.meta;
            if let syn::Meta::NameValue(name_value) = meta {
                if let syn::Expr::Lit(lit) = &name_value.value
                    && let syn::Lit::Str(s) = &lit.lit
                {
                    return Ok(Some(s.value()));
                }
            } else {
                return Ok(None);
            }
        }
    }

    Ok(None)
}

fn find_attr_alias_type(field: &Field) -> Option<Result<String, syn::Error>> {
    let mut flag = false;
    for attr in &field.attrs {
        if attr.path().is_ident("gts_type") {
            flag = true;
            let meta = &attr.meta;
            if let syn::Meta::NameValue(name_value) = meta
                && let syn::Expr::Lit(lit_int) = &name_value.value
                && let syn::Lit::Str(lit_int) = &lit_int.lit
            {
                return Some(Ok(lit_int.to_token_stream().to_string()));
            }
        }
    }
    if flag {
        Some(Err(syn::Error::new(
            field.span(),
            "Unable to parse gts type",
        )))
    } else {
        None
    }
}
