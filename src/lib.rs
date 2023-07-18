#![feature(path_file_prefix)]
use base64::{engine::general_purpose, Engine as _};
use std::{ffi::OsStr, panic, path::Path};
use urlencoding::decode;

use worker::*;

extern crate console_error_panic_hook;

fn console_error(e: Error) {
    console_error!("{}", e);
}

async fn post_put(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let mut req_mut = _req.clone_mut().map_err(|e| console_log!("{}", e)).unwrap();
    let form_data = req_mut.form_data().await.map_err(console_error).unwrap();
    let form_entry = form_data
        .get("upload")
        .unwrap_or_else(|| form_data.get("paste").unwrap());
    let file = match form_entry {
        FormEntry::Field(form_entry) => File::new(form_entry.into_bytes(), "paste"),
        FormEntry::File(form_entry) => form_entry,
    };
    let filename = file.name();
    let path = Path::new(filename.as_str())
        .file_prefix()
        .unwrap_or_else(|| OsStr::new(""))
        .to_str()
        .unwrap_or_else(|| "");
    let path_str = path;
    if path_str == "/" {
        return Response::ok("cannot update /");
    }
    let b64 = general_purpose::STANDARD.encode(&file.bytes().await.unwrap());
    let _result = ctx
        .kv("rust_worker")
        .map_err(console_error)
        .unwrap()
        .put(path_str, b64)
        .map_err(|e| console_log!("{}", e))
        .unwrap()
        .execute()
        .await;
    let url = _req.url().map_err(console_error).unwrap();
    let redirect = String::from(url) + path_str;
    let redirect_url = Url::parse(redirect.as_str()).unwrap();
    Response::redirect(redirect_url)
}

async fn delete(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let empty_string = String::new();
    let file = ctx.param("file").unwrap_or_else(|| &empty_string).as_str();
    let _result = ctx
        .kv("rust_worker")
        .map_err(|e| console_log!("{}", e))
        .unwrap()
        .delete(file)
        .await;
    Response::ok("deleted")
}

async fn get_index(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let _result = ctx
        .kv("rust_worker")
        .map_err(console_error)
        .unwrap()
        .get("/")
        .text()
        .await
        .map_err(|e| console_log!("{}", e))
        .unwrap()
        .unwrap_or_else(|| String::from("404"));
    Response::from_html(_result)
}

async fn get(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let reqpath = String::from(decode(ctx.param("file").unwrap()).expect("UTF-8"));
    let path = Path::new(reqpath.as_str());
    let name = path
        .file_prefix()
        .unwrap_or_else(|| OsStr::new(""))
        .to_str()
        .unwrap_or_else(|| "");
    let _result = ctx
        .kv("rust_worker")
        .map_err(console_error)
        .unwrap()
        .get(name)
        .text()
        .await
        .map_err(|e| console_log!("{}", e))
        .unwrap_or_else(|_| Some(String::from("404")))
        .unwrap_or_else(|| String::from("404"));
    let body = general_purpose::STANDARD
        .decode(_result.as_str())
        .unwrap_or_else(|_| "".as_bytes().to_vec());
    return match _result.as_str() {
        "404" => Response::error(_result, 404),
        &_ => {
            let ext = path
                .extension()
                .unwrap_or_else(|| OsStr::new(""))
                .to_str()
                .unwrap_or_else(|| "");
            match ext {
                "json" => {
                    let response = Response::from_body(ResponseBody::Body(body));
                    let mut headers = Headers::new();
                    let _result = headers.append("Content-type", "application/json").unwrap();
                    Ok(Response::with_headers(response.unwrap(), headers))
                }
                &_ => Response::from_body(ResponseBody::Body(body)),
            }
        }
    };
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let router = Router::new();
    router
        .get_async("/", get_index)
        .get_async("/:file", get)
        .post_async("/", post_put)
        .put_async("/", post_put)
        .delete_async("/:file", delete)
        .get_async("/delete/:file", delete)
        .or_else_any_method_async("/", |req, _ctx| async move {
            Response::redirect(req.url().unwrap())
        })
        .run(req, env)
        .await
}
