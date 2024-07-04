pub fn os_id() -> String {
    let os_id = sysinfo::System::distribution_id();
    match os_id.as_str() {
        "centos" | "rocky" => "rhel".to_owned(),
        _ => os_id,
    }
}

pub fn os_major_version() -> String {
    let os_version = sysinfo::System::os_version().expect("version id not found");
    match os_version.find('.') {
        Some(i) => os_version[..i].to_owned(),
        None => os_version,
    }
}

pub fn cpu_arch() -> String {
    let cpu_arch = sysinfo::System::cpu_arch().expect("can't know cpu arch");
    match cpu_arch.as_str() {
        "aarch64" | "arm64" => "arm64",
        "x86" | "x86_64" | "amd64" => "amd64",
        _ => return cpu_arch,
    }
    .to_owned()
}
