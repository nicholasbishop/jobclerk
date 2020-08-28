use actix_web::body::Body;
use actix_web::{middleware, App, HttpServer};
use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use env_logger::Env;
use fehler::throws;
use jobclerk_server::{api, ui};
use jobclerk_server::{make_pool, Pool, DEFAULT_POSTGRES_PORT};
use log::error;

#[derive(Template)]
#[template(path = "internal_error.html")]
struct InternalErrorTemplate {}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("db error: {0}")]
    Db(#[from] tokio_postgres::Error),

    #[error("server error: {0}")]
    Server(#[from] jobclerk_server::Error),

    #[error("pool error: {0}")]
    Pool(#[from] bb8::RunError<tokio_postgres::Error>),

    #[error("template error: {0}")]
    Template(#[from] askama::Error),
}

impl actix_web::ResponseError for Error {
    fn error_response(&self) -> HttpResponse<Body> {
        error!("internal error: {}", self);
        let template = InternalErrorTemplate {};
        let body = match template.render() {
            Ok(body) => body,
            Err(err) => {
                error!("template error: {}", err);
                "error: failed to render error!".into()
            }
        };
        HttpResponse::InternalServerError().body(body)
    }
}

#[throws]
async fn list_projects(pool: web::Data<Pool>) -> impl Responder {
    HttpResponse::Ok().body(ui::list_projects(pool.get_ref()).await?)
}

#[throws]
async fn get_project(
    pool: web::Data<Pool>,
    path: web::Path<(String,)>,
) -> impl Responder {
    let project_name = &path.0;
    HttpResponse::Ok()
        .body(ui::get_project(pool.get_ref(), project_name).await?)
}

async fn handle_api_request(
    pool: web::Data<Pool>,
    req: web::Json<jobclerk_types::Request>,
) -> impl Responder {
    HttpResponse::Ok().json(api::handle_request(pool.get_ref(), &req).await)
}

pub fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .route("/projects", web::get().to(list_projects))
            .route("/projects/{project_name}", web::get().to(get_project))
            .route("/api", web::post().to(handle_api_request)),
    );
}

#[throws(anyhow::Error)]
#[actix_rt::main]
async fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    let pool = make_pool(DEFAULT_POSTGRES_PORT).await?;

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .configure(app_config)
            .data(pool.clone())
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await?;
}
