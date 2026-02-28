fn main() {
    let bytes: [u8; 4] = [0, 0, 0, 0];
    let mut samples = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let array: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
        samples.push(f32::from_le_bytes(array));
    }
    println!("{:?}", samples);
}
