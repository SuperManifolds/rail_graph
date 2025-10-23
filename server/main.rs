use actix_files::Files;
use actix_web::{App, HttpServer, middleware, web, HttpResponse, Result};
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct ChangelogRelease {
    tag_name: String,
    name: String,
    body: String,
    published_at: String,
}

fn parse_changelog(content: &str) -> Vec<ChangelogRelease> {
    let lines: Vec<&str> = content.lines().collect();
    let mut releases = Vec::new();

    // Find all header lines (# version - date)
    let mut i = 0;
    while i < lines.len() {
        if lines[i].starts_with("# ") && !lines[i].contains("Unreleased") {
            let header = lines[i];

            // Parse header: "# v0.1.2 - 2025-10-22"
            if let Some(header_content) = header.strip_prefix("# ") {
                let parts: Vec<&str> = header_content.split(" - ").collect();
                if parts.len() == 2 {
                    let tag_name = parts[0].to_string();
                    let published_at = parts[1].to_string();

                    // Extract body content until next header or end
                    let body_start = i + 1;
                    let body_end = lines[body_start..]
                        .iter()
                        .position(|line| line.starts_with("# "))
                        .map(|pos| body_start + pos)
                        .unwrap_or(lines.len());

                    let body = lines[body_start..body_end]
                        .join("\n")
                        .trim()
                        .to_string();

                    releases.push(ChangelogRelease {
                        tag_name: tag_name.clone(),
                        name: tag_name,
                        body,
                        published_at,
                    });

                    i = body_end;
                    continue;
                }
            }
        }
        i += 1;
    }

    releases
}

async fn changelog() -> Result<HttpResponse> {
    match std::fs::read_to_string("./CHANGELOG.md") {
        Ok(content) => {
            let releases = parse_changelog(&content);
            Ok(HttpResponse::Ok().json(releases))
        },
        Err(_) => Ok(HttpResponse::NotFound().body("Changelog not found")),
    }
}

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
            .route("/api/changelog", web::get().to(changelog))
            .service(Files::new("/", "./dist").index_file("index.html"))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
