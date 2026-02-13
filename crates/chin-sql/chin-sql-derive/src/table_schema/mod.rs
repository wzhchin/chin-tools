mod fieldhandler;

use std::collections::HashMap;

use fieldhandler::parse_field_info;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Field, Fields, parse_macro_input};

use crate::table_schema::fieldhandler::{FieldInfo, KeyOrder};

pub(crate) fn generate_table_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new(
                    input.span(),
                    "GenerateTable can only be applied to structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new(input.span(), "GenerateTable can only be applied to structs")
                .to_compile_error()
                .into();
        }
    };

    let table_name = camel2snake(struct_name.to_string().as_str());
    let mut constants = TokenStream2::default();
    let mut table_struct = TokenStream2::default();
    let table_struct_ident = format_ident!("{}Table", struct_name);
    let mut table_struct_fields = TokenStream2::default();

    for f in fields.into_iter() {
        let field_ident = f.ident.as_ref().unwrap();
        let field_indent = format_ident!("{}", field_ident.to_string().to_uppercase());
        let field_name_str = field_ident.to_string();
        let ty = f.ty.clone();
        constants.extend(quote! { pub const #field_indent: &'static str = #field_name_str; });

        table_struct_fields.extend(quote! {
            #[inline]
            pub fn #field_ident(&'a self) -> chin_sql::SqlTypedField<'a, #ty> {
                chin_sql::SqlTypedField::new(self.alias, #field_name_str)
            }
        });
    }

    table_struct.extend(quote! {
        pub struct #table_struct_ident<'a> {
            pub alias: &'a str
        }

        impl<'a> chin_sql::SqlPlainTable<'a> for #table_struct_ident<'a> {
                fn table_name(&self) -> &'static str {
                    #table_name
                }

                fn alias(&self) -> &'a str {
                    self.alias
                }
        }

        impl<'a> #table_struct_ident<'a> {
            pub fn new(alias: &'a str) -> Self {
                Self {
                    alias: alias.into()
                }
            }

            // name with alias
            pub fn nwa(&self) -> String {
                format!("{} {}", #table_name, self.alias)
            }

            pub fn all_fields(&self) -> String {
                format!("{} *", self.alias)
            }

            #[inline]
            pub fn table(&self) -> &'static str {
                #table_name
            }

            #table_struct_fields
        }
    });

    let functions = match generate_functions(&table_name, &fields.iter().collect()) {
        Ok(ok) => ok,
        Err(err) => {
            return syn::Error::new(input.span(), format!("GenerateTableSchema error: {err}"))
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        #table_struct

        impl #struct_name {
            pub const TABLE: &'static str = #table_name;
            #constants

            #functions
        }

    };

    TokenStream::from(expanded)
}

fn generate_functions(table_name: &str, fields: &Vec<&Field>) -> Result<TokenStream2, syn::Error> {
    let mut tokens = TokenStream2::new();
    let field_infos: Result<Vec<(FieldInfo, &syn::Field)>, syn::Error> = fields
        .iter()
        .map(|field| parse_field_info(field).map(|fi| (fi, *field)))
        .collect();
    let field_infos = field_infos?;

    tokens.extend(generate_inner(table_name, &field_infos)?);

    Ok(tokens)
}

fn generate_inner(
    table_name: &str,
    fields: &Vec<(FieldInfo, &Field)>,
) -> Result<TokenStream2, syn::Error> {
    let inserter = to_sql_inserter(fields);

    let mut column_structs = TokenStream2::new();
    let mut all_fields = TokenStream2::new();
    for (fi, _) in fields {
        let column_name = &fi.column_name;
        let not_null = fi.not_null;
        let sql_type = &match fi.field_type {
            chin_sql::LogicFieldType::Bool => quote! { chin_sql::LogicFieldType::Bool },
            chin_sql::LogicFieldType::I8 => quote! { chin_sql::LogicFieldType::I8 },
            chin_sql::LogicFieldType::I16 => quote! { chin_sql::LogicFieldType::I16 },
            chin_sql::LogicFieldType::I32 => quote! { chin_sql::LogicFieldType::I3 },
            chin_sql::LogicFieldType::I64 => quote! { chin_sql::LogicFieldType::I64 },
            chin_sql::LogicFieldType::F64 => quote! { chin_sql::LogicFieldType::F64 },
            chin_sql::LogicFieldType::Varchar(c) => {
                quote! { chin_sql::LogicFieldType::Varchar(#c) }
            }
            chin_sql::LogicFieldType::Text => quote! { chin_sql::LogicFieldType::Text },
            chin_sql::LogicFieldType::Blob => quote! { chin_sql::LogicFieldType::Blob },
            chin_sql::LogicFieldType::Timestamptz => {
                quote! { chin_sql::LogicFieldType::Timestamptz }
            }
            chin_sql::LogicFieldType::Timestamp => quote! {chin_sql::LogicFieldType::Timestamp },
        };

        column_structs.extend(quote! {
            chin_sql::CreateTableField {
                name: #column_name,
                kind: #sql_type,
                not_null: #not_null,
            },
        });
        all_fields.extend(quote! {#column_name, });
    }

    fn all_same_order<T, F>(eles: &[T], f: F) -> i32
    where
        F: Fn(&T) -> KeyOrder,
    {
        if eles.iter().map(&f).all(|c| matches!(c, KeyOrder::Default)) {
            return 1;
        }
        if eles.iter().map(f).all(|c| matches!(c, KeyOrder::Num(_))) {
            return 2;
        }
        0
    }
    let fields: Vec<(&FieldInfo, &Field)> = fields.iter().map(|(fi, f)| (fi, *f)).collect();

    let pkey_fields: Vec<_> = fields
        .iter()
        .filter(|(e1, _)| e1.pkey.is_some())
        .copied()
        .collect();
    if all_same_order(&pkey_fields, |e| e.0.pkey.unwrap()) == 0 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Primary KeyOrder should be the same".to_string(),
        ));
    }

    let mut functions = TokenStream2::new();

    let mut pkey_schema = TokenStream2::new();
    for (fi, _) in &pkey_fields {
        let cn = fi.column_name.as_str();
        pkey_schema.extend(quote! { #cn });
        pkey_schema.extend(quote! {, });
    }
    functions.extend(key_func("pkey", &pkey_fields));

    let mut unikey_map = HashMap::new();
    let mut key_map = HashMap::new();
    for (fi, field) in fields {
        for (key_name, (unique, order)) in fi.key_map.iter() {
            if *unique {
                unikey_map.entry(key_name).or_insert(vec![]).push((
                    &fi.column_name,
                    order,
                    field,
                    fi,
                ));
            } else {
                key_map
                    .entry(key_name)
                    .or_insert(vec![])
                    .push((&fi.column_name, order, field, fi));
            }
        }
    }

    let mut unikey_schema = TokenStream2::new();
    for (key, vs) in unikey_map {
        let mut fs: Vec<(&FieldInfo, &Field)> = vs.iter().map(|(_, _, f, fi)| (*fi, *f)).collect();
        if all_same_order(&vs, |f| *f.1) == 1 {
        } else if all_same_order(&vs, |f| *f.1) == 2 {
            fs.sort_by(|e1, e2| e1.0.pkey.unwrap().order().cmp(&e2.0.pkey.unwrap().order()));
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("KeyOrder should be the same, {key}"),
            ));
        }
        functions.extend(key_func(format!("unikey_{key}").as_str(), &fs));
        let fss: Vec<String> = fs.iter().map(|f| f.0.column_name.clone()).collect();
        let fss = fss.join(", ");
        let key_name = format!("ukey_{key}");
        unikey_schema.extend(quote! {  ( #key_name, &[#fss]), });
    }

    let mut key_schema = TokenStream2::new();
    for (key, vs) in key_map {
        let mut fs: Vec<(&FieldInfo, &Field)> = vs.iter().map(|(_, _, f, fi)| (*fi, *f)).collect();
        if all_same_order(&vs, |f| *f.1) == 1 {
        } else if all_same_order(&vs, |f| *f.1) == 2 {
            fs.sort_by(|e1, e2| e1.0.pkey.unwrap().order().cmp(&e2.0.pkey.unwrap().order()));
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("KeyOrder should be the same, {key}"),
            ));
        }
        functions.extend(key_func(format!("key_{key}").as_str(), &fs));
        let fss: Vec<String> = fs.iter().map(|f| f.0.column_name.clone()).collect();
        let fss = fss.join(", ");
        let key_name = format!("key_{key}");
        key_schema.extend(quote! {  ( #key_name, &[#fss]), });
    }

    Ok(quote! {
        #[inline]
        pub fn create_sql() -> &'static chin_sql::CreateTableSql {
            &chin_sql::CreateTableSql {
                table_name: #table_name,
                fields: &[ #column_structs ],
                pkey: &[ #pkey_schema ],
                unikeys: &[ #unikey_schema ],
                keys: &[ #key_schema ]
            }
        }

        pub fn all_field_names() -> &'static [&'static str] {
            &[ #all_fields ]
        }

        #inserter

        #functions
    })
}

fn camel2snake(name: &str) -> String {
    let mut table_name = String::new();
    let chars: Vec<char> = name.to_string().chars().collect();
    for i in 0..chars.len() {
        if i == 0 {
            table_name.push(chars[i].to_ascii_lowercase());
            continue;
        }

        let cur = chars.get(i).unwrap();
        let last = chars.get(i - 1).unwrap().is_ascii_uppercase();
        let next = chars.get(i + 1).is_none_or(|c| c.is_ascii_uppercase());

        if cur.is_ascii_uppercase() && (!last || !next) {
            table_name.push('_');
        }
        table_name.push(cur.to_ascii_lowercase());
    }

    table_name
}

fn to_sql_inserter(fields: &Vec<(FieldInfo, &Field)>) -> TokenStream2 {
    let mut func_stream = TokenStream2::default();
    for (fi, f) in fields.iter() {
        let db_field_ident = format_ident!("{}", fi.column_name.to_uppercase());
        let Some(field_indent) = f.ident.clone() else {
            return syn::Error::new(f.span(), "this field has no ident").to_compile_error();
        };
        if let Some(mp) = fi.to_sql_func.as_ref() {
            let mp = format_ident!("{}", mp);
            func_stream.extend(quote! { .field(Self::#db_field_ident, #mp(self.#field_indent)) });
        } else {
            func_stream.extend(quote! { .field(Self::#db_field_ident, self.#field_indent) });
        }
    }

    quote! {
        pub fn to_sql_inserter(self) -> chin_sql::SqlInserter<'static> {
            chin_sql::SqlInserter::new(Self::TABLE)
                #func_stream
        }
    }
}

fn key_func(prefix: &str, fields: &Vec<(&FieldInfo, &Field)>) -> TokenStream2 {
    let mut args = TokenStream2::default();
    let mut wheres = TokenStream2::default();
    let len = fields.len();
    for (n, (fi, field)) in fields.iter().enumerate() {
        let Some(field_name) = field.ident.clone() else {
            return syn::Error::new(field.span(), "field ident is empty").into_compile_error();
        };
        let tty = field.ty.clone();
        args.extend(quote! { #field_name: #tty });
        if n < len - 1 {
            args.extend(quote! {, });
        }
        let column_name = format_ident!("{}", &fi.column_name.as_str().to_uppercase());

        wheres.extend(quote! { chin_sql::Wheres::equal(Self::#column_name, #field_name), });
    }
    let reader = format_ident!("{}_reader", prefix);
    let updater = format_ident!("{}_updater", prefix);
    let where_cond = format_ident!("{}_cond", prefix);

    let expanded = quote! {
        pub fn #reader<'a>(#args) -> chin_sql::SqlBuilder<'a> {
            chin_sql::SqlBuilder::read_all(Self::TABLE)
            .r#where(chin_sql::Wheres::and([
                #wheres
            ]))
        }

        pub fn #updater<'c>(#args) -> chin_sql::SqlUpdater<'c> {
            chin_sql::SqlUpdater::new(Self::TABLE)
            .r#where(chin_sql::Wheres::and([
                #wheres
            ]))
        }

        pub fn #where_cond<'c>(#args) -> chin_sql::Wheres<'c> {
            chin_sql::Wheres::and([
                #wheres
            ])
        }
    };

    expanded
}
