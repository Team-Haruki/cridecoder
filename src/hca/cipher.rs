//! HCA cipher/encryption handling

/// Initialize cipher table for type 0 (no encryption)
pub fn cipher_init_0(cipher_table: &mut [u8; 256]) {
    for (i, entry) in cipher_table.iter_mut().enumerate() {
        *entry = i as u8;
    }
}

/// Initialize cipher table for type 1 (simple scramble)
pub fn cipher_init_1(cipher_table: &mut [u8; 256]) {
    const MUL: u32 = 13;
    const ADD: u32 = 11;
    let mut v: u32 = 0;

    for entry in cipher_table.iter_mut().take(255).skip(1) {
        v = (v * MUL + ADD) & 0xFF;
        if v == 0 || v == 0xFF {
            v = (v * MUL + ADD) & 0xFF;
        }
        *entry = v as u8;
    }
    cipher_table[0] = 0;
    cipher_table[0xFF] = 0xFF;
}

fn cipher_init_56_create_table(r: &mut [u8; 16], key: u8) {
    let mul = ((key & 1) << 3) | 5;
    let add = (key & 0xE) | 1;
    let mut key = key >> 4;

    for entry in r.iter_mut() {
        key = (key.wrapping_mul(mul).wrapping_add(add)) & 0xF;
        *entry = key;
    }
}

/// Initialize cipher table for type 56 (key-based encryption)
pub fn cipher_init_56(cipher_table: &mut [u8; 256], keycode: u64) {
    let mut kc = [0u8; 8];
    let mut seed = [0u8; 16];
    let mut base = [0u8; 256];
    let mut base_r = [0u8; 16];
    let mut base_c = [0u8; 16];

    let keycode = if keycode != 0 { keycode - 1 } else { keycode };

    for (r, entry) in kc.iter_mut().enumerate().take(7) {
        *entry = ((keycode >> (r * 8)) & 0xFF) as u8;
    }

    seed[0x00] = kc[1];
    seed[0x01] = kc[1] ^ kc[6];
    seed[0x02] = kc[2] ^ kc[3];
    seed[0x03] = kc[2];
    seed[0x04] = kc[2] ^ kc[1];
    seed[0x05] = kc[3] ^ kc[4];
    seed[0x06] = kc[3];
    seed[0x07] = kc[3] ^ kc[2];
    seed[0x08] = kc[4] ^ kc[5];
    seed[0x09] = kc[4];
    seed[0x0A] = kc[4] ^ kc[3];
    seed[0x0B] = kc[5] ^ kc[6];
    seed[0x0C] = kc[5];
    seed[0x0D] = kc[5] ^ kc[4];
    seed[0x0E] = kc[6] ^ kc[1];
    seed[0x0F] = kc[6];

    cipher_init_56_create_table(&mut base_r, kc[0]);
    for r in 0..16 {
        cipher_init_56_create_table(&mut base_c, seed[r]);
        let nb = base_r[r] << 4;
        for c in 0..16 {
            base[r * 16 + c] = nb | base_c[c];
        }
    }

    let mut x: usize = 0;
    let mut pos = 1;
    for _ in 0..256 {
        x = (x + 17) & 0xFF;
        if base[x] != 0 && base[x] != 0xFF {
            cipher_table[pos] = base[x];
            pos += 1;
        }
    }
    cipher_table[0] = 0;
    cipher_table[0xFF] = 0xFF;
}

/// Initialize cipher table based on type
pub fn cipher_init(cipher_table: &mut [u8; 256], ciph_type: u32, keycode: u64) {
    let ciph_type = if ciph_type == 56 && keycode == 0 {
        0
    } else {
        ciph_type
    };

    match ciph_type {
        0 => cipher_init_0(cipher_table),
        1 => cipher_init_1(cipher_table),
        56 => cipher_init_56(cipher_table, keycode),
        _ => cipher_init_0(cipher_table),
    }
}

/// Decrypt data in-place using cipher table
pub fn cipher_decrypt(cipher_table: &[u8; 256], data: &mut [u8]) {
    for byte in data.iter_mut() {
        *byte = cipher_table[*byte as usize];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cipher_init_0() {
        let mut table = [0u8; 256];
        cipher_init_0(&mut table);
        for i in 0..256 {
            assert_eq!(table[i], i as u8);
        }
    }

    #[test]
    fn test_cipher_init_1() {
        let mut table = [0u8; 256];
        cipher_init_1(&mut table);
        assert_eq!(table[0], 0);
        assert_eq!(table[0xFF], 0xFF);
        // Check that middle values are scrambled
        assert_ne!(table[1], 1);
    }
}
