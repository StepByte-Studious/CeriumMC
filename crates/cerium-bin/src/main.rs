//! The CeriumMC server executable.

fn main() {
    println!("{}", banner());
}

/// The startup line printed before anything else runs.
fn banner() -> String {
    format!("CeriumMC {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::banner;

    #[test]
    fn banner_contains_name_and_version() {
        assert_eq!(banner(), concat!("CeriumMC ", env!("CARGO_PKG_VERSION")));
    }
}
