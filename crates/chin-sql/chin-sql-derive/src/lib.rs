use proc_macro::TokenStream;

mod table_schema;

#[proc_macro_derive(
    GenerateTableSchema,
    attributes(gts_primary, gts_type, gts_key, gts_unique, gts_tosql)
)]
pub fn generate_table_schema(input: TokenStream) -> TokenStream {
    table_schema::generate_table_schema(input)
}
