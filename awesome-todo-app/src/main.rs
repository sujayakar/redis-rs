use actix_web::{web, App, HttpResponse, HttpServer, Result};
use cache_helper::CacheHelper;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Todo {
    id: String,
    title: String,
    completed: bool,
    created_at: DateTime<Utc>,
}

struct AppState {
    cache: Mutex<CacheHelper>,
    todos: Mutex<Vec<Todo>>,
}

#[derive(Deserialize)]
struct CreateTodo {
    title: String,
}

async fn get_todos(data: web::Data<AppState>) -> Result<HttpResponse> {
    let todos = data.todos.lock().unwrap();
    Ok(HttpResponse::Ok().json(&*todos))
}

async fn create_todo(
    data: web::Data<AppState>,
    todo: web::Json<CreateTodo>,
) -> Result<HttpResponse> {
    let new_todo = Todo {
        id: Uuid::new_v4().to_string(),
        title: todo.title.clone(),
        completed: false,
        created_at: Utc::now(),
    };
    
    // Cache the new todo
    let cache_key = {
        let mut cache = data.cache.lock().unwrap();
        cache.cache_value(&new_todo).unwrap_or_else(|e| {
            eprintln!("Failed to cache todo: {}", e);
            new_todo.id.clone()
        })
    };
    
    println!("Cached todo with key: {}", cache_key);
    
    // Store in memory
    let mut todos = data.todos.lock().unwrap();
    todos.push(new_todo.clone());
    
    Ok(HttpResponse::Created().json(&new_todo))
}

async fn get_todo(data: web::Data<AppState>, path: web::Path<String>) -> Result<HttpResponse> {
    let todo_id = path.into_inner();
    
    // Try to get from cache first
    let mut cache = data.cache.lock().unwrap();
    if let Ok(Some(todo)) = cache.get_cached::<Todo>(&todo_id) {
        println!("Found todo in cache!");
        return Ok(HttpResponse::Ok().json(&todo));
    }
    
    // Fall back to memory storage
    let todos = data.todos.lock().unwrap();
    if let Some(todo) = todos.iter().find(|t| t.id == todo_id) {
        Ok(HttpResponse::Ok().json(todo))
    } else {
        Ok(HttpResponse::NotFound().body("Todo not found"))
    }
}

async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "uuid_version": "0.8", // This will need to be updated to 1.0
    })))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    println!("Starting Awesome Todo App...");
    println!("Connecting to Redis...");
    
    // IMPORTANT: This uses Redis for caching
    // Students will need to have Redis running locally
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    
    let cache = CacheHelper::new(&redis_url)
        .expect("Failed to connect to Redis");
    
    let app_state = web::Data::new(AppState {
        cache: Mutex::new(cache),
        todos: Mutex::new(Vec::new()),
    });
    
    println!("Server starting at http://127.0.0.1:8080");
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/health", web::get().to(health_check))
            .route("/todos", web::get().to(get_todos))
            .route("/todos", web::post().to(create_todo))
            .route("/todos/{id}", web::get().to(get_todo))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}