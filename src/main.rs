//! lxsrcs-rs：洛雪音乐自定义源服务端（Rust 实现，兼容 Go 版 `/api/v0` 线格式）。
//!
//! 启动：`lxsrcs [配置路径]`，默认读当前目录 `conf.toml`。

mod access;
mod auth;
mod cache;
mod config;
mod crypto;
mod itype;
mod lyric;
mod resp;
mod routes;
mod sources;
mod state;
mod util;

use std::sync::Arc;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let conf_path = std::env::args().nth(1).unwrap_or_else(|| "conf.toml".to_string());
    let cfg = match config::Config::load(&conf_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("配置错误: {err}");
            std::process::exit(1);
        }
    };

    let level = if cfg.main.debug { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(format!("lxsrcs={level},info")));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let listen = cfg.main.listen.clone();
    let state = state::AppState::new(cfg);
    let app = routes::build(Arc::clone(&state));

    let listener = match tokio::net::TcpListener::bind(&listen).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("监听 {listen} 失败: {err}");
            std::process::exit(1);
        }
    };

    access::set_console(state.cfg.main.console);
    let platforms = sources::PLATFORMS.iter().filter(|p| sources::qualities(&state.cfg, p).is_some()).count();
    access::log_startup(&listen, &state.version, platforms);
    println!("监听 127.0.0.1:{}", listen.rsplit(':').next().unwrap_or("1101"));
    println!("自定义源脚本: http://{listen}/api/v0/lx-script.js");

    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("服务异常退出: {err}");
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("收到退出信号，正在关闭");
}
