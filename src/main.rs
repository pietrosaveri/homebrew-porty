use clap::{Parser, Subcommand};
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::*;
use anyhow::{Context, Result};

#[derive(Parser)]
#[command(name = "porty", version, about = "Local port inspector")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    /// Show verbose output including executable paths
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable colored output (green for dev, red for unknown, yellow for system)
    #[arg(short, long, global = true)]
    colors: bool,
}

#[derive(Subcommand)]
enum Cmd {
    All,
    Dev,
    Port { port: u16 },
    Free { port: u16 },
    Kill {
        port: u16,
        /// Skip confirmation and kill immediately
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Debug, Clone)]
struct PortEntry {
    port: u16,
    pid: Option<u32>,
    process: Option<String>,
    exec_path: Option<String>,
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Dev,
    Database,
    Container,
    System,
    Unknown,
}

fn filter_default(entries: &[PortEntry]) -> Vec<PortEntry> {
    entries.iter()
        .filter(|e| matches!(e.kind, Kind::Dev | Kind::Unknown))
        .cloned()
        .collect()
}

fn filter_dev(entries: &[PortEntry]) -> Vec<PortEntry> {
    entries.iter()
        .filter(|e| matches!(e.kind, Kind::Dev))
        .cloned()
        .collect()
}

fn classify(port: u16, process: Option<&str>) -> Kind {
    // Process-based rules take priority (more accurate)
    if let Some(p) = process {
        let p = p.to_lowercase();

        // macOS system processes (check first to avoid misclassification)
        if p.contains("launchd") || p.contains("mdnsresponder") || p.contains("cups")
            || p.contains("controlcenter") || p.contains("airplay") {
            return Kind::System;
        }

        // Dev servers
        if p.contains("node") || p.contains("vite") || p.contains("next")
            || p.contains("python") || p.contains("ruby") || p.contains("rails")
            || p.contains("django") || p.contains("flask") || p.contains("phoenix")
            || p.contains("webpack") || p.contains("npm") || p.contains("yarn")
            || p.contains("puma") || p.contains("unicorn") {
            return Kind::Dev;
        }

        // Databases
        if p.contains("postgres") || p.contains("mysql") || p.contains("redis")
            || p.contains("mongod") || p.contains("mariadb") || p.contains("couchdb") {
            return Kind::Database;
        }

        // Containers
        if p.contains("docker") || p.contains("containerd") || p.contains("colima")
            || p.contains("podman") {
            return Kind::Container;
        }
    }

    // Port-based rules (fallback when process is unknown or doesn't match)
    match port {
        3000 | 5173 | 8080 | 8000 | 4200 | 3001 | 5000 | 9000 => Kind::Dev,
        5432 | 3306 | 6379 | 27017 | 1433 | 5984 => Kind::Database,
        2375 | 2376 => Kind::Container,
        631 => Kind::System,
        _ => Kind::Unknown,
    }
}

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::{thread, time::Duration};

fn print_banner(colors: bool) {
    const BANNER: &str = include_str!("../banner.txt");

    if colors {
        // Rainbow colors for each line
        let rainbow_colors = [
            Color::Red,
            Color::Yellow,
            Color::Green,
            Color::Cyan,
            Color::Blue,
            Color::Magenta,
        ];

        for (i, line) in BANNER.lines().enumerate() {
            let color = rainbow_colors[i % rainbow_colors.len()];
            let colored_line = match color {
                Color::Red => format!("\x1b[31m{}\x1b[0m", line),
                Color::Yellow => format!("\x1b[33m{}\x1b[0m", line),
                Color::Green => format!("\x1b[32m{}\x1b[0m", line),
                Color::Cyan => format!("\x1b[36m{}\x1b[0m", line),
                Color::Blue => format!("\x1b[34m{}\x1b[0m", line),
                Color::Magenta => format!("\x1b[35m{}\x1b[0m", line),
                _ => line.to_string(),
            };
            println!("{}", colored_line);
        }
    } else {
        println!("{}", BANNER);
    }
}

fn kill_pid(pid: u32) -> anyhow::Result<()> {
    let pid = Pid::from_raw(pid as i32);
    kill(pid, Signal::SIGTERM)?;
    thread::sleep(Duration::from_millis(300));

    if kill(pid, None).is_ok() {
        kill(pid, Signal::SIGKILL)?;
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let entries = discover_ports().unwrap_or_else(|e| {
        eprintln!("discovery error: {e}");
        vec![]
    });

    match cli.cmd {
        None => {
            print_banner(cli.colors);
            let filtered = filter_default(&entries);
            print_table(filtered, cli.verbose, cli.colors);
        }
        Some(Cmd::All) => {
            print_banner(cli.colors);
            print_table(entries, cli.verbose, cli.colors);
        }
        Some(Cmd::Dev) => {
            print_banner(cli.colors);
            let filtered = filter_dev(&entries);
            print_table(filtered, cli.verbose, cli.colors);
        }
        Some(Cmd::Port { port }) => {
            print_banner(cli.colors);
            cmd_port(&entries, port, cli.verbose, cli.colors);
        }
        Some(Cmd::Free { port }) => {
            cmd_free(&entries, port);
        }
        Some(Cmd::Kill { port, force }) => {
            cmd_kill(&entries, port, force);
        }
    }
}

fn cmd_port(entries: &[PortEntry], port: u16, verbose: bool, colors: bool) {
    let found: Vec<_> = entries.iter().cloned().filter(|e| e.port == port).collect();
    if found.is_empty() {
        println!("No listener found on port {port}");
    } else {
        print_table(found, verbose, colors);
    }
}

fn cmd_free(entries: &[PortEntry], port: u16) {
    let found: Vec<_> = entries.iter().filter(|e| e.port == port).collect();
    if found.is_empty() {
        println!("No TCP listener found on port {port}");
    } else {
        println!("Port {port} is in use:");
        for entry in found {
            if let (Some(pid), Some(process)) = (entry.pid, &entry.process) {
                println!("  {} (PID {})", process, pid);
                println!("  Hint: kill {} or use 'porty kill {}'", pid, port);
            }
        }
    }
}

fn cmd_kill(entries: &[PortEntry], port: u16, force: bool) {
    let found: Vec<_> = entries.iter().filter(|e| e.port == port).collect();
    if found.is_empty() {
        println!("No process found on port {port}");
        return;
    }

    // Deduplicate by PID to avoid killing the same process twice
    let mut target_pids: Vec<(u32, String)> = Vec::new();
    let mut seen_pids = std::collections::HashSet::new();

    for entry in &found {
        if let (Some(pid), Some(process)) = (entry.pid, &entry.process) {
            if seen_pids.insert(pid) {
                target_pids.push((pid, process.clone()));
            }
        }
    }

    if target_pids.is_empty() {
        println!("No killable process found on port {port}");
        return;
    }

    // Show what would be killed
    println!("Process(es) on port {port}:");
    for (pid, process) in &target_pids {
        println!("  {} (PID {})", process, pid);
    }

    if !force {
        println!("\nDry run mode. Use --force to actually kill the process(es).");
        println!("Example: porty kill {} --force", port);
        return;
    }

    // Actually kill with --force
    println!("\nKilling process(es)...");
    for (pid, process) in target_pids {
        println!("Killing {} (PID {})...", process, pid);
        match kill_pid(pid) {
            Ok(_) => println!("Process killed"),
            Err(e) => eprintln!("Failed to kill process: {}", e),
        }
    }
}

#[cfg(target_os = "macos")]
fn discover_ports() -> Result<Vec<PortEntry>> {
    use std::process::Command;
    
    // Use lsof -F for reliable port→PID mapping
    // -F: field output (parseable)
    // -n: no DNS lookups
    // -P: numeric ports
    // -iTCP: TCP only
    // -sTCP:LISTEN: only LISTEN state
    // Output format:
    //   p<pid>
    //   c<command>
    //   n<address>:<port>
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"])
        .output()
        .context("failed to run lsof (is it installed?)")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "lsof exited with status {}",
            output.status
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    
    let mut current_pid: Option<u32> = None;
    let mut current_cmd: Option<String> = None;

    // Parse lsof -F output
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }

        let field_type = line.chars().next().unwrap();
        let value = &line[1..];

        match field_type {
            'p' => {
                // PID field
                current_pid = value.parse::<u32>().ok();
                current_cmd = None; // reset for new process
            }
            'c' => {
                // Command name (from lsof, as fallback)
                current_cmd = Some(value.to_string());
            }
            'n' => {
                // Network address field (e.g., "*:3000" or "127.0.0.1:8080")
                if let Some(pid) = current_pid {
                    // Extract port from address
                    if let Some(port) = extract_port(value) {
                        // Get process info from libproc
                        let process = get_process_name_libproc(pid)
                            .or_else(|| current_cmd.clone());
                        let exec_path = get_exec_path_libproc(pid);

                        let kind = classify(port, process.as_deref());

                        entries.push(PortEntry {
                            port,
                            pid: Some(pid),
                            process,
                            exec_path,
                            kind,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    // Deduplicate by (port, pid) - same process might have IPv4 + IPv6 listeners
    let mut seen = std::collections::HashSet::new();
    let unique_entries: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            if let Some(pid) = e.pid {
                seen.insert((e.port, pid))
            } else {
                true
            }
        })
        .collect();

    let mut result = unique_entries;
    result.sort_by_key(|e| e.port);
    Ok(result)
}

#[cfg(target_os = "macos")]
fn extract_port(addr: &str) -> Option<u16> {
    // Handle formats like:
    // *:3000
    // 127.0.0.1:8080
    // [::1]:5432 (IPv6)
    // 3000 (LISTEN) - parse only leading digits
    
    let colon_pos = addr.rfind(':')?;
    let after = &addr[colon_pos + 1..];
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(target_os = "macos")]
fn get_process_name_libproc(pid: u32) -> Option<String> {
    // Use libproc native API to get process name
    // proc_name only needs a small buffer (not the full path buffer)
    const PROC_NAME_SIZE: usize = 256;
    let mut buffer = [0u8; PROC_NAME_SIZE];
    let ret = unsafe {
        libc::proc_name(
            pid as i32,
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as u32,
        )
    };

    if ret > 0 {
        let name_bytes = &buffer[..ret as usize];
        if let Ok(name) = std::str::from_utf8(name_bytes) {
            let clean_name = name.trim_end_matches('\0').to_string();
            if !clean_name.is_empty() {
                return Some(clean_name);
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn get_exec_path_libproc(pid: u32) -> Option<String> {
    // Use libproc to get the full executable path
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let ret = unsafe {
        libc::proc_pidpath(
            pid as i32,
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len() as u32,
        )
    };
    if ret <= 0 {
        return None;
    }
    let bytes = &buffer[..ret as usize];
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

#[cfg(not(target_os = "macos"))]
fn discover_ports() -> Result<Vec<PortEntry>> {
    Err(anyhow::anyhow!("This tool only supports macOS"))
}

fn print_table(entries: Vec<PortEntry>, verbose: bool, colors: bool) {
    if entries.is_empty() {
        println!("No ports found.");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.apply_modifier(UTF8_ROUND_CORNERS);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_width(100);

    if verbose {
        table.set_header(vec!["PORT", "PROCESS", "CATEGORY", "PID", "EXEC PATH"]);
    } else {
        table.set_header(vec!["PORT", "PROCESS", "CATEGORY", "PID"]);
    }

    for e in entries {
        let category_cell = if colors {
            Cell::new(format_kind(e.kind))
                .fg(get_kind_color(e.kind))
        } else {
            Cell::new(format_kind(e.kind))
        };

        if verbose {
            table.add_row(vec![
                Cell::new(e.port),
                Cell::new(e.process.unwrap_or("-".into())),
                category_cell,
                Cell::new(e.pid.map(|p| p.to_string()).unwrap_or("-".into())),
                Cell::new(e.exec_path.unwrap_or("-".into())),
            ]);
        } else {
            table.add_row(vec![
                Cell::new(e.port),
                Cell::new(e.process.unwrap_or("-".into())),
                category_cell,
                Cell::new(e.pid.map(|p| p.to_string()).unwrap_or("-".into())),
            ]);
        }
    }

    println!("{table}");
}

fn format_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Dev => "Dev Server",
        Kind::Database => "Database",
        Kind::Container => "Container",
        Kind::System => "System",
        Kind::Unknown => "Unknown",
    }
}

fn get_kind_color(kind: Kind) -> Color {
    match kind {
        Kind::Dev => Color::Green,
        Kind::Database => Color::Cyan,
        Kind::Container => Color::Blue,
        Kind::System => Color::Yellow,
        Kind::Unknown => Color::Red,
    }
}
