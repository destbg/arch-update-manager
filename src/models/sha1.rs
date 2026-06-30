pub struct Sha1 {
    state: [u32; 5],
    buffer: Vec<u8>,
    length: u64,
}

impl Sha1 {
    pub fn new() -> Self {
        return Sha1 {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
            buffer: Vec::with_capacity(64),
            length: 0,
        };
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);

        if !self.buffer.is_empty() {
            let needed = 64 - self.buffer.len();
            let take = needed.min(data.len());
            self.buffer.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buffer.len() == 64 {
                let block: [u8; 64] = self.buffer[..].try_into().unwrap();
                self.process(&block);
                self.buffer.clear();
            }
        }

        let mut chunks = data.chunks_exact(64);
        for chunk in &mut chunks {
            let block: [u8; 64] = chunk.try_into().unwrap();
            self.process(&block);
        }
        self.buffer.extend_from_slice(chunks.remainder());
    }

    pub fn finalize_hex(mut self) -> String {
        let bit_length = self.length.wrapping_mul(8);
        self.buffer.push(0x80);
        while self.buffer.len() % 64 != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_length.to_be_bytes());

        let buffer = std::mem::take(&mut self.buffer);
        for chunk in buffer.chunks(64) {
            let block: [u8; 64] = chunk.try_into().unwrap();
            self.process(&block);
        }

        let mut out = String::with_capacity(40);
        for value in self.state {
            out.push_str(&format!("{:08x}", value));
        }
        return out;
    }

    fn process(&mut self, chunk: &[u8; 64]) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];

        for (i, word) in w.iter().enumerate() {
            let (f, k) = if i < 20 {
                ((b & c) | ((!b) & d), 0x5A827999u32)
            } else if i < 40 {
                (b ^ c ^ d, 0x6ED9EBA1)
            } else if i < 60 {
                ((b & c) | (b & d) | (c & d), 0x8F1BBCDC)
            } else {
                (b ^ c ^ d, 0xCA62C1D6)
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
    }
}
