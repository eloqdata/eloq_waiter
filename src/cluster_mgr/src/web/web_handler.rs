use crate::web::{Response, SUPPORT_CMD};
use actix_web::{get, web, HttpResponse, Responder};
use reqwest::StatusCode;

#[get("/check_health")]
pub(crate) async fn check_health() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("I'm OK.")
}

#[get("/{cluster}/{command}/status")]
pub(crate) async fn check_cmd_status(
    cluster: web::Path<String>,
    command: web::Path<String>,
) -> impl Responder {
    let cmd_str = command.clone().to_lowercase();
    if cmd_str.is_empty() || !SUPPORT_CMD.contains(&cmd_str.as_str()) {
        let support_cmd_list = SUPPORT_CMD.join(",");
        HttpResponse::build(StatusCode::BAD_REQUEST)
            .content_type("application/json")
            .json(Response {
                code: 400,
                msg: format!(
                    "un support command = {cmd_str}, for now support command list {support_cmd_list}"
                ),
                data: Default::default(),
            })
    } else {
        println!("{cluster} {command}");
        HttpResponse::Ok()
            .content_type("application/json")
            .json(Response::succ_def())
    }
}
