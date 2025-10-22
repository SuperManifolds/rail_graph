use actix_files::Files;
use actix_web::{App, HttpServer, middleware};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Get port from environment or default to 8080
    let port = env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    println!("Starting server on 0.0.0.0:{port}");

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Compress::default())
            .wrap(middleware::Logger::default())
            .service(Files::new("/", "./dist").index_file("index.html"))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
