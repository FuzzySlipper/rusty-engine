use std::time::Instant;

fn decode(path: &str) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

fn time<F: FnMut() -> usize>(name: &str, w: u32, h: u32, mut f: F) {
    let mut size = 0;
    for _ in 0..3 { size = f(); }
    let runs = 20;
    let mut times: Vec<f64> = (0..runs).map(|_| { let t = Instant::now(); size = f(); t.elapsed().as_secs_f64() * 1000.0 }).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{w}x{h}\t{name}\t{:.2} ms median\t{:.2} ms p90\t{} bytes\t{:.1} MB/s at 60fps", times[runs/2], times[runs*9/10], size, size as f64 * 60.0 / 1e6);
}

fn main() {
    for path in std::env::args().skip(1) {
        let (w, h, rgba) = decode(&path);
        time("raw-rgba", w, h, || { let v = rgba.clone(); v.len() });
        for q in [70u8, 80, 90] {
            time(&format!("jpeg-encoder q{q} rgba"), w, h, || {
                let mut out = Vec::with_capacity(rgba.len() / 8);
                jpeg_encoder::Encoder::new(&mut out, q).encode(&rgba, w as u16, h as u16, jpeg_encoder::ColorType::Rgba).unwrap();
                out.len()
            });
        }
        time("webp-lossless (image-webp)", w, h, || {
            let mut out = Vec::new();
            image_webp::WebPEncoder::new(&mut out).encode(&rgba, w, h, image_webp::ColorType::Rgba8).unwrap();
            out.len()
        });
        time("png fast", w, h, || {
            let mut out = Vec::new();
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_compression(png::Compression::Fastest);
            enc.write_header().unwrap().write_image_data(&rgba).unwrap();
            out.len()
        });
        time("lz4 raw", w, h, || lz4_flex::compress(&rgba).len());
        time("zstd-1 raw", w, h, || zstd::bulk::compress(&rgba, 1).unwrap().len());
    }
}
