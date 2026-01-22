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
    /// Show all listening ports
    All,
    /// Show only dev servers (node etc.)
    Dev,
    /// Show dev servers and containers
    Prod,
    /// Show process info for a specific port
    Port { port: u16 },
    /// Check if a port is available
    Free { port: u16 },
    /// Kill the process on a specific port
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

#[derive(Debug, Clone)]
struct DetailedPortInfo {
    port: u16,
    pid: u32,
    process_name: String,
    command: String,
    working_dir: Option<String>,
    exec_path: Option<String>,
    user_name: String,
    uid: u32,
    parent_chain: Vec<(u32, String)>,
    children: Vec<(u32, String)>,
    uptime: String,
    start_time: String,
    memory_rss: u64,      // in KB
    memory_virtual: u64,  // in KB
    cpu_usage: f64,
    thread_count: u32,
    file_descriptors: u32,
    listen_addresses: Vec<String>,
    active_connections: u32,
    other_ports: Vec<u16>,
    env_vars: Vec<(String, String)>,
    kind: Kind,
    docker_info: Option<DockerInfo>,
}

#[derive(Debug, Clone)]
struct DockerInfo {
    container_id: String,
    container_name: String,
    image: String,
    status: String,
    volumes: Vec<String>,
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

fn filter_prod(entries: &[PortEntry]) -> Vec<PortEntry> {
    entries.iter()
        .filter(|e| matches!(e.kind, Kind::Dev | Kind::Container))
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
        Some(Cmd::Prod) => {
            print_banner(cli.colors);
            let filtered = filter_prod(&entries);
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
        // Get detailed info for the first matching entry
        if let Some(entry) = found.first() {
            if let Some(pid) = entry.pid {
                if let Ok(detailed) = get_detailed_port_info(port, pid, entry.kind) {
                    print_detailed_port_info(&detailed, colors);
                    return;
                }
            }
        }
        // Fallback to table view
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
fn get_detailed_port_info(port: u16, pid: u32, kind: Kind) -> Result<DetailedPortInfo> {
    use std::thread;
    
    let process_name = get_process_name_libproc(pid).unwrap_or_else(|| "unknown".to_string());
    let exec_path = get_exec_path_libproc(pid);
    
    // Run expensive operations in parallel
    let pid_for_ps = pid;
    let pid_for_lsof = pid;
    let pid_for_children = pid;
    let port_for_connections = port;
    let process_name_for_docker = process_name.clone();
    
    // Thread 1: Combined ps call for all process info
    let ps_handle = thread::spawn(move || {
        get_combined_ps_info(pid_for_ps)
    });
    
    // Thread 2: Combined lsof call for all file/network info
    let lsof_handle = thread::spawn(move || {
        get_combined_lsof_info(pid_for_lsof, port)
    });
    
    // Thread 3: Parent chain (requires multiple calls)
    let parent_handle = thread::spawn(move || {
        get_parent_chain(pid)
    });
    
    // Thread 4: Child processes
    let children_handle = thread::spawn(move || {
        get_child_processes(pid_for_children)
    });
    
    // Thread 5: Active connections
    let connections_handle = thread::spawn(move || {
        count_active_connections(port_for_connections)
    });
    
    // Thread 6: Docker info (only if it looks like a container)
    let docker_handle = thread::spawn(move || {
        get_docker_info(port_for_connections, &process_name_for_docker)
    });
    
    // Collect results
    let ps_info = ps_handle.join().unwrap_or_default();
    let lsof_info = lsof_handle.join().unwrap_or_default();
    let parent_chain = parent_handle.join().unwrap_or_default();
    let children = children_handle.join().unwrap_or_default();
    let active_connections = connections_handle.join().unwrap_or(0);
    let docker_info = docker_handle.join().unwrap_or(None);

    Ok(DetailedPortInfo {
        port,
        pid,
        process_name,
        command: ps_info.command.unwrap_or_else(|| "unknown".to_string()),
        working_dir: lsof_info.working_dir,
        exec_path,
        user_name: ps_info.user_name,
        uid: ps_info.uid,
        parent_chain,
        children,
        uptime: ps_info.uptime,
        start_time: ps_info.start_time,
        memory_rss: ps_info.memory_rss,
        memory_virtual: ps_info.memory_virtual,
        cpu_usage: ps_info.cpu_usage,
        thread_count: ps_info.thread_count,
        file_descriptors: lsof_info.file_descriptors,
        listen_addresses: lsof_info.listen_addresses,
        active_connections,
        other_ports: lsof_info.other_ports,
        env_vars: ps_info.env_vars,
        kind,
        docker_info,
    })
}

#[derive(Default)]
struct CombinedPsInfo {
    command: Option<String>,
    user_name: String,
    uid: u32,
    uptime: String,
    start_time: String,
    memory_rss: u64,
    memory_virtual: u64,
    cpu_usage: f64,
    thread_count: u32,
    env_vars: Vec<(String, String)>,
}

#[derive(Default)]
struct CombinedLsofInfo {
    working_dir: Option<String>,
    file_descriptors: u32,
    listen_addresses: Vec<String>,
    other_ports: Vec<u16>,
}

#[cfg(target_os = "macos")]
fn get_combined_ps_info(pid: u32) -> CombinedPsInfo {
    use std::process::Command;
    
    let mut info = CombinedPsInfo::default();
    info.user_name = "unknown".to_string();
    info.uptime = "unknown".to_string();
    info.start_time = "unknown".to_string();
    
    // Single ps call for most info: command, user, uid, rss, vsz, %cpu, etime, lstart
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command=,user=,uid=,rss=,vsz=,%cpu=,etime="])
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Parse from the end since command can contain spaces
            let parts: Vec<&str> = text.rsplitn(7, char::is_whitespace).collect();
            if parts.len() >= 6 {
                info.uptime = parts[0].trim().to_string();
                info.cpu_usage = parts[1].trim().parse().unwrap_or(0.0);
                info.memory_virtual = parts[2].trim().parse().unwrap_or(0);
                info.memory_rss = parts[3].trim().parse().unwrap_or(0);
                info.uid = parts[4].trim().parse().unwrap_or(0);
                info.user_name = parts[5].trim().to_string();
                // Command is everything before these fields
                if parts.len() >= 7 {
                    let cmd_parts: Vec<&str> = parts[6..].iter().rev().map(|s| *s).collect();
                    info.command = Some(cmd_parts.join(" "));
                }
            }
        }
    }
    
    // Get full command separately (the above parsing can be tricky)
    let cmd_output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output();
    
    if let Ok(output) = cmd_output {
        if output.status.success() {
            let cmd = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !cmd.is_empty() {
                info.command = Some(cmd);
            }
        }
    }
    
    // Get lstart (start time) separately since it has spaces
    let lstart_output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .output();
    
    if let Ok(output) = lstart_output {
        if output.status.success() {
            info.start_time = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    
    // Get thread count
    let thread_output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-M"])
        .output();
    
    if let Ok(output) = thread_output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            info.thread_count = text.lines().count().saturating_sub(1).max(1) as u32;
        }
    }
    
    // Get environment variables
    info.env_vars = get_environment_variables(pid);
    
    info
}

#[cfg(target_os = "macos")]
fn get_combined_lsof_info(pid: u32, current_port: u16) -> CombinedLsofInfo {
    use std::process::Command;
    
    let mut info = CombinedLsofInfo::default();
    
    // Single lsof call for all file info
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut ports_seen = std::collections::HashSet::new();
            
            for line in text.lines() {
                if line.starts_with('n') {
                    let value = &line[1..];
                    
                    // Check for cwd
                    if info.working_dir.is_none() && !value.contains(':') && value.starts_with('/') {
                        // This might be cwd, but we need to verify
                    }
                    
                    // Check for listening addresses
                    if value.contains(':') {
                        if let Some(port) = extract_port(value) {
                            if port == current_port {
                                info.listen_addresses.push(value.to_string());
                            } else {
                                ports_seen.insert(port);
                            }
                        }
                    }
                }
                
                info.file_descriptors += 1;
            }
            
            info.other_ports = ports_seen.into_iter().collect();
            info.other_ports.sort();
        }
    }
    
    // Get working directory with specific lsof call (more reliable)
    let cwd_output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-a", "-d", "cwd", "-Fn"])
        .output();
    
    if let Ok(output) = cwd_output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.starts_with('n') {
                    info.working_dir = Some(line[1..].to_string());
                    break;
                }
            }
        }
    }
    
    info
}

#[cfg(target_os = "macos")]
fn get_parent_chain(pid: u32) -> Vec<(u32, String)> {
    let mut chain = Vec::new();
    let mut current_pid = pid;
    let mut seen = std::collections::HashSet::new();
    
    // Limit to 10 levels to avoid infinite loops
    for _ in 0..10 {
        if seen.contains(&current_pid) {
            break;
        }
        seen.insert(current_pid);
        
        if let Some(parent_pid) = get_parent_pid(current_pid) {
            if parent_pid == 0 || parent_pid == 1 {
                break;
            }
            if let Some(name) = get_process_name_libproc(parent_pid) {
                chain.insert(0, (parent_pid, name));
                current_pid = parent_pid;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    chain
}

#[cfg(target_os = "macos")]
fn get_parent_pid(pid: u32) -> Option<u32> {
    use std::process::Command;
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "ppid="])
        .output()
        .ok()?;
    
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_child_processes(pid: u32) -> Vec<(u32, String)> {
    use std::process::Command;
    let output = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output();
    
    let Ok(output) = output else {
        return Vec::new();
    };
    
    if !output.status.success() {
        return Vec::new();
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut children = Vec::new();
    
    for line in text.lines() {
        if let Ok(child_pid) = line.trim().parse::<u32>() {
            if let Some(name) = get_process_name_libproc(child_pid) {
                children.push((child_pid, name));
            }
        }
    }
    
    children
}

#[cfg(target_os = "macos")]
fn count_active_connections(port: u16) -> u32 {
    use std::process::Command;
    let output = Command::new("lsof")
        .args(["-iTCP", &format!(":{}", port), "-sTCP:ESTABLISHED", "-t"])
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return text.lines().count() as u32;
        }
    }
    0
}

#[cfg(target_os = "macos")]
fn get_environment_variables(pid: u32) -> Vec<(String, String)> {
    use std::process::Command;
    
    // Get important environment variables
    let important_vars = vec![
        "NODE_ENV", "PORT", "DATABASE_URL", "RAILS_ENV", "FLASK_ENV",
        "DJANGO_SETTINGS_MODULE", "PYTHON_ENV", "GO_ENV", "RUST_ENV",
        "PATH", "HOME", "USER", "PWD", "LANG"
    ];
    
    let output = Command::new("ps")
        .args(["eww", &pid.to_string()])
        .output();
    
    let Ok(output) = output else {
        return Vec::new();
    };
    
    if !output.status.success() {
        return Vec::new();
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut env_vars = Vec::new();
    
    // Parse ps eww output which shows environment variables
    // Format is: "VAR1=value1 VAR2=value2 ..."
    for line in text.lines() {
        for part in line.split_whitespace() {
            if let Some(eq_pos) = part.find('=') {
                let key = &part[..eq_pos];
                let value = &part[eq_pos + 1..];
                
                // Only keep important vars to avoid clutter
                if important_vars.contains(&key) {
                    env_vars.push((key.to_string(), value.to_string()));
                }
            }
        }
    }
    
    env_vars
}

#[cfg(target_os = "macos")]
fn get_docker_info(port: u16, process_name: &str) -> Option<DockerInfo> {
    // Only check if this looks like a Docker process
    if !process_name.to_lowercase().contains("docker") 
        && !process_name.to_lowercase().contains("com.docker") {
        return None;
    }
    
    use std::process::Command;
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}|{{.Mounts}}|{{.Ports}}"])
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(6, '|').collect();
        if parts.len() < 6 {
            continue;
        }
        
        let ports_str = parts[5];
        
        // Check if this container exposes our port
        if ports_str.contains(&format!(":{}", port)) || ports_str.contains(&format!("->{}/ ", port)) {
            let volumes: Vec<String> = parts[4]
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect();
            
            return Some(DockerInfo {
                container_id: parts[0].to_string(),
                container_name: parts[1].to_string(),
                image: parts[2].to_string(),
                status: parts[3].to_string(),
                volumes,
            });
        }
    }
    
    None
}

fn print_detailed_port_info(info: &DetailedPortInfo, colors: bool) {
    let header_color = if colors { "\x1b[1;36m" } else { "" };
    let label_color = if colors { "\x1b[1m" } else { "" };
    let section_color = if colors { "\x1b[1;34m" } else { "" }; // Blue for section titles
    let kind_color = if colors {
        match info.kind {
            Kind::Dev => "\x1b[32m",
            Kind::Database => "\x1b[36m",
            Kind::Container => "\x1b[34m",
            Kind::System => "\x1b[33m",
            Kind::Unknown => "\x1b[31m",
        }
    } else {
        ""
    };
    let reset = if colors { "\x1b[0m" } else { "" };
    
    // Header
    println!();
    println!("{}╭─────────────────────────────────────────────────────────────────────╮{}", header_color, reset);
    println!("{}│ Port {} - Process Details{}{}", header_color, info.port, " ".repeat(43 - info.port.to_string().len()), reset);
    println!("{}╰─────────────────────────────────────────────────────────────────────╯{}", header_color, reset);
    println!();
    
    // Process Information
    println!("{}PROCESS INFORMATION{}", section_color, reset);
    println!("  {}Name:{} {}", label_color, reset, info.process_name);
    println!("  {}PID:{} {}", label_color, reset, info.pid);
    println!("  {}Category:{} {}{}{}", label_color, reset, kind_color, format_kind(info.kind), reset);
    println!("  {}Command:{} {}", label_color, reset, info.command);
    
    if let Some(ref dir) = info.working_dir {
        println!("  {}Directory:{} {}", label_color, reset, dir);
    }
    
    if let Some(ref path) = info.exec_path {
        println!("  {}Exec Path:{} {}", label_color, reset, path);
    }
    
    println!("  {}User:{} {} ({})", label_color, reset, info.user_name, info.uid);
    println!("  {}Uptime:{} {} (started {})", label_color, reset, info.uptime, info.start_time);
    println!();
    
    // Process Tree
    if !info.parent_chain.is_empty() || !info.children.is_empty() {
        println!("{}PROCESS TREE{}", section_color, reset);
        
        if !info.parent_chain.is_empty() {
            let chain_str = info.parent_chain
                .iter()
                .map(|(pid, name)| format!("{} ({})", name, pid))
                .collect::<Vec<_>>()
                .join(" → ");
            println!("  {}Parents:{} {} → {} ({})", 
                label_color, reset, chain_str, info.process_name, info.pid);
        } else {
            println!("  {}Parents:{} None", label_color, reset);
        }
        
        if !info.children.is_empty() {
            let children_str = info.children
                .iter()
                .map(|(pid, name)| format!("{} ({})", name, pid))
                .collect::<Vec<_>>()
                .join(", ");
            println!("  {}Children:{} {}", label_color, reset, children_str);
        } else {
            println!("  {}Children:{} None", label_color, reset);
        }
        println!();
    }
    
    // Resources
    println!("{}RESOURCES{}", section_color, reset);
    println!("  {}Memory:{} {} MB (RSS), {} MB (Virtual)", 
        label_color, reset,
        format_mb(info.memory_rss),
        format_mb(info.memory_virtual)
    );
    println!("  {}CPU:{} {}%", label_color, reset, format_float(info.cpu_usage, 1));
    println!("  {}Threads:{} {}", label_color, reset, info.thread_count);
    println!("  {}File Descriptors:{} {} open", label_color, reset, info.file_descriptors);
    println!();
    
    // Network
    println!("{}NETWORK{}", section_color, reset);
    
    if !info.listen_addresses.is_empty() {
        let binding_str = info.listen_addresses.join(", ");
        let (ipv4, ipv6): (Vec<_>, Vec<_>) = info.listen_addresses
            .iter()
            .partition(|addr| !addr.contains('['));
        
        if !ipv4.is_empty() && !ipv6.is_empty() {
            let ipv4_str: Vec<String> = ipv4.iter().map(|s| s.to_string()).collect();
            let ipv6_str: Vec<String> = ipv6.iter().map(|s| s.to_string()).collect();
            println!("  {}Binding:{} {} (IPv4) + {} (IPv6)", 
                label_color, reset,
                ipv4_str.join(", "),
                ipv6_str.join(", ")
            );
        } else {
            println!("  {}Binding:{} {}", label_color, reset, binding_str);
        }
    } else {
        println!("  {}Binding:{} *:{}", label_color, reset, info.port);
    }
    
    println!("  {}Protocol:{} TCP (LISTEN)", label_color, reset);
    println!("  {}Connections:{} {} active", label_color, reset, info.active_connections);
    
    if !info.other_ports.is_empty() {
        let ports_str = info.other_ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {}Other Ports:{} Also listening on {}", label_color, reset, ports_str);
    }
    println!();
    
    // Environment Variables
    if !info.env_vars.is_empty() {
        println!("{}ENVIRONMENT{}", section_color, reset);
        for (key, value) in info.env_vars.iter().take(10) {
            // Truncate only PATH since it's typically very long
            let display_value = if key == "PATH" && value.len() > 100 {
                format!("{}...", &value[..97])
            } else {
                value.clone()
            };
            println!("  {}={}", key, display_value);
        }
        if info.env_vars.len() > 10 {
            println!("  ({} more environment variables)", info.env_vars.len() - 10);
        }
        println!();
    }
    
    // Docker Info
    if let Some(ref docker) = info.docker_info {
        println!("{}CONTAINER INFORMATION{}", section_color, reset);
        println!("  {}Container:{} {}", label_color, reset, docker.container_name);
        println!("  {}ID:{} {}", label_color, reset, docker.container_id);
        println!("  {}Image:{} {}", label_color, reset, docker.image);
        println!("  {}Status:{} {}", label_color, reset, docker.status);
        
        if !docker.volumes.is_empty() {
            println!("  {}Volumes:{}", label_color, reset);
            for vol in &docker.volumes {
                println!("    - {}", vol);
            }
        }
        println!();
    }
}

fn format_mb(kb: u64) -> String {
    let mb = kb as f64 / 1024.0;
    format!("{:.1}", mb)
}

fn format_float(val: f64, decimals: usize) -> String {
    format!("{:.1$}", val, decimals)
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

    // Enrich container entries with Docker container names
    enrich_docker_containers(&mut result);

    result.sort_by_key(|e| e.port);
    Ok(result)
}

#[cfg(target_os = "macos")]
fn enrich_docker_containers(entries: &mut [PortEntry]) {
    use std::process::Command;

    // Query Docker for all running containers with their ports, names, and images
    // Format: <container_id>|<name>|<image>|<ports>
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.ID}}|{{.Names}}|{{.Image}}|{{.Ports}}"])
        .output();

    let Ok(output) = output else {
        // Docker not available or not running
        return;
    };

    if !output.status.success() {
        return;
    }

    let text = String::from_utf8_lossy(&output.stdout);

    // Build a map of port -> (container name, image)
    let mut port_to_container: std::collections::HashMap<u16, (String, String)> = std::collections::HashMap::new();

    for line in text.lines() {
        if line.is_empty() {
            continue;
        }

        // Parse the format: container_id|name|image|ports
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() < 4 {
            continue;
        }

        let container_name = parts[1];
        let image = parts[2];
        let ports_str = parts[3];

        // Parse ports from Docker format: "0.0.0.0:8080->80/tcp, 0.0.0.0:8443->443/tcp"
        // We want to extract the host port (e.g., 8080, 8443)
        for port_mapping in ports_str.split(',') {
            let port_mapping = port_mapping.trim();

            // Look for patterns like "0.0.0.0:6379->6379/tcp" or ":::6379->6379/tcp"
            if let Some(arrow_pos) = port_mapping.find("->") {
                let before_arrow = &port_mapping[..arrow_pos];

                // Extract the host port (after the last colon before ->)
                if let Some(colon_pos) = before_arrow.rfind(':') {
                    let port_str = &before_arrow[colon_pos + 1..];
                    if let Ok(port) = port_str.parse::<u16>() {
                        port_to_container.insert(port, (container_name.to_string(), image.to_string()));
                    }
                }
            }
        }
    }

    // Enrich entries that are Docker processes
    for entry in entries.iter_mut() {
        if entry.kind == Kind::Container {
            if let Some(process) = &entry.process {
                // Check if it's a Docker process
                if process.to_lowercase().contains("docker") {
                    // Look up the container name for this port
                    if let Some((container_name, image)) = port_to_container.get(&entry.port) {
                        // Try to get a friendly name from the image
                        let friendly_name = get_friendly_container_name(container_name, image);
                        entry.process = Some(friendly_name);
                    } else {
                        // No Docker container found, try to guess based on port
                        if let Some(service_name) = guess_service_by_port(entry.port) {
                            entry.process = Some(format!("{} (container)", service_name));
                        }
                    }
                }
            }
        }
    }
}

/// Get a friendly container name from the container name and image
fn get_friendly_container_name(container_name: &str, image: &str) -> String {
    // Extract the base image name (e.g., "redis" from "redis:7-alpine")
    let image_base = image
        .split(':')
        .next()
        .unwrap_or(image)
        .split('/')
        .last()
        .unwrap_or(image);

    // Use the image base name if it's more descriptive than the container name
    let display_name = if is_generic_name(container_name) && !is_generic_name(image_base) {
        image_base
    } else {
        container_name
    };

    format!("{} (container)", display_name)
}

/// Check if a name is generic/auto-generated
fn is_generic_name(name: &str) -> bool {
    // Docker auto-generated names or hash-like names
    name.len() > 20 || name.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Guess the service type based on well-known ports
fn guess_service_by_port(port: u16) -> Option<&'static str> {
    match port {
        // PostgreSQL
        5432 => Some("postgresql"),
        // MySQL/MariaDB
        3306 => Some("mysql"),
        // Redis
        6379 => Some("redis"),
        // MongoDB
        27017 => Some("mongodb"),
        // Neo4j
        7474 => Some("neo4j-http"),
        7473 => Some("neo4j-https"),
        7687 => Some("neo4j-bolt"),
        // Elasticsearch
        9200 => Some("elasticsearch"),
        9300 => Some("elasticsearch-cluster"),
        // RabbitMQ
        5672 => Some("rabbitmq"),
        15672 => Some("rabbitmq-mgmt"),
        // Memcached
        11211 => Some("memcached"),
        // CouchDB
        5984 => Some("couchdb"),
        // Cassandra
        9042 => Some("cassandra"),
        // InfluxDB
        8086 => Some("influxdb"),
        // Kafka
        9092 => Some("kafka"),
        // MinIO
        9000 => Some("minio"),
        9001 => Some("minio-console"),
        _ => None,
    }
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
