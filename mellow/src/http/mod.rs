use actix_web::{ App, HttpServer };

mod routes;

pub async fn start() -> std::io::Result<()> {
	HttpServer::new(|| App::new().configure(routes::configure))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}