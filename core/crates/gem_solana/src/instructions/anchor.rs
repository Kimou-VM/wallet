use sha2::{Digest, Sha256};

const DISCRIMINATOR_LEN: usize = 8;

fn namespaced_discriminator(namespace: &str, name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b":");
    hasher.update(name.as_bytes());
    let hash = hasher.finalize();
    let mut data = [0u8; 8];
    data.copy_from_slice(&hash[..8]);
    data
}

pub fn global_discriminator(name: &str) -> [u8; 8] {
    namespaced_discriminator("global", name)
}

pub fn account_body<'a>(data: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let (discriminator, body) = data.split_at_checked(DISCRIMINATOR_LEN)?;
    (discriminator == namespaced_discriminator("account", name)).then_some(body)
}

#[cfg(test)]
mod tests {
    use hex_lit::hex;

    use super::*;

    #[test]
    fn test_global_discriminator() {
        assert_eq!(global_discriminator("init_order"), hex!("204c290c27a284db"));
    }

    #[test]
    fn test_account_body() {
        assert_eq!(account_body(&hex!("3f95d10ce18063090102"), "Whirlpool"), Some([1, 2].as_slice()));
        assert_eq!(account_body(&hex!("3f95d10ce18063090102"), "TickArray"), None);
        assert_eq!(account_body(&hex!("3f95d10c"), "Whirlpool"), None);
    }
}
