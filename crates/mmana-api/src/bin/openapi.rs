//! Prints the OpenAPI document; the desktop app generates its TypeScript types from it.

use utoipa::OpenApi;

fn main() {
    println!("{}", mmana_api::ApiDoc::openapi().to_pretty_json().expect("serialisable OpenAPI document"));
}
