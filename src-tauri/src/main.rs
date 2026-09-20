#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--mode")
        && args.windows(2).any(|pair| pair[0] == "--mode" && pair[1] == "readonly")
    {
        if let Err(error) = tunneldock_lib::readonly_mcp::run_stdio() {
            eprintln!("TunnelDock read-only MCP server failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    tunneldock_lib::run();
}
