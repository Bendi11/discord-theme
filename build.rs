use humantime::format_rfc3339_seconds;
use std::time::SystemTime;

fn main() {
    println!(
        "cargo:rustc-env=COMPILEDATE={}",
        format_rfc3339_seconds(SystemTime::now())
            .to_string()
            .split('T')
            .next()
            .unwrap()
    )
}
