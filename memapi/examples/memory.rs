use psutil;
use sysinfo::{ProcessExt, SystemExt};

fn main() {
    let mut system = sysinfo::System::new_with_specifics(sysinfo::RefreshKind::everything());
    system.refresh_memory();
    let pid = sysinfo::get_current_pid().unwrap();
    system.refresh_process(pid);
    let process = system.get_process(pid).unwrap();
    println!("My PID: {}", pid);
    println!(
        "Available: {}, free: {}, used: {}, total: {}",
        system.get_available_memory(),
        system.get_free_memory(),
        system.get_used_memory(),
        system.get_total_memory()
    );
    println!(
        "Available memory per psutil: {:?}",
        psutil::memory::virtual_memory().unwrap()
    );
    println!(
        "Process memory, per sysinfo: {}, process virtual memory: {}",
        process.memory(),
        process.virtual_memory()
    );
    let process = psutil::process::Process::current().unwrap();
    let memory = process.memory_info().unwrap();
    println!(
        "Process memroy, per psutil. rss: {}, vms: {}",
        memory.rss(),
        memory.vms()
    );
    std::thread::sleep_ms(1000 * 1000);
}
