// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--mcp`: servidor MCP por stdio para os agentes (multiplexador do ALFRED), sem abrir janela.
    if std::env::args().any(|a| a == "--mcp") {
        app_lib::run_mcp();
        return;
    }
    app_lib::run();
}
