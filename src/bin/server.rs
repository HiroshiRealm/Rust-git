use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use rust_git::repository::{bundle, Repository};

#[derive(Clone)]
struct AppState {
    repo_path: Arc<PathBuf>,
}

#[tokio::main]
async fn main() {
    // Expect the path to the repository to serve as a command-line argument.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: server <path-to-git-repo>");
        std::process::exit(1);
    }
    let repo_path = PathBuf::from(&args[1]);
    if !repo_path.join(".git").is_dir() {
        eprintln!("Error: Provided path is not a valid git repository.");
        std::process::exit(1);
    }

    let state = AppState {
        repo_path: Arc::new(repo_path),
    };

    let app = Router::new()
        .route("/repo.bundle", get(handle_fetch))
        .route("/repo.bundle", post(handle_push))
        .with_state(state.clone());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {}", addr);
    println!("Serving repository at: {}", state.repo_path.display());

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Handler for fetch (client GETs a bundle)
async fn handle_fetch(State(state): State<AppState>) -> Response {
    match Repository::open(state.repo_path.as_ref()) {
        Ok(repo) => {
            let mut buffer = Vec::new();
            match bundle::create_bundle(&repo, &mut buffer) {
                Ok(_) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/octet-stream")],
                    buffer,
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create bundle: {}", e),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to open repository: {}", e),
        )
            .into_response(),
    }
}

// Handler for push (client POSTs a bundle)
async fn handle_push(State(state): State<AppState>, body: Bytes) -> Response {
    match Repository::open(state.repo_path.as_ref()) {
        Ok(repo) => {
            let reader = std::io::Cursor::new(body);
            match bundle::unbundle(&repo, reader, None) {
                Ok(_) => (StatusCode::OK, "Push successful".to_string()).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to unbundle: {}", e),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to open repository: {}", e),
        )
            .into_response(),
    }
} 